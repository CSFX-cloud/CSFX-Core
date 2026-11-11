use crate::builds::Builds;
use crate::detect::Mode;
use crate::error::ServiceError;
use crate::job::{runtime_request, RuntimeSpec};
use crate::routes;
use chrono::Utc;
use entity::{site_deployments, site_domains, sites, SiteDeployments, SiteDomains, Sites};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, Set,
};
use std::collections::BTreeMap;
use std::time::Duration;
use uuid::Uuid;

const RETIRE_POLL: Duration = Duration::from_secs(5);
const RETIRE_ATTEMPTS: u32 = 60;
const MAX_LABEL_LENGTH: usize = 40;

pub async fn load(db: &DatabaseConnection, id: Uuid) -> Result<sites::Model, ServiceError> {
    Sites::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)
}

pub fn mode_of(site: &sites::Model) -> Result<Mode, ServiceError> {
    match site.mode.as_str() {
        "static" => Ok(Mode::Static),
        "container" => Ok(Mode::Container),
        _ => Err(ServiceError::Invalid(
            "site has an unknown mode".to_string(),
        )),
    }
}

pub fn preview_hostname(
    pull_request: Option<i32>,
    branch: &str,
    site_name: &str,
    domain: &str,
) -> String {
    let label = match pull_request {
        Some(number) => format!("pr-{number}"),
        None => slug(branch),
    };
    format!("{label}-{site_name}.{domain}")
}

fn slug(value: &str) -> String {
    let mapped: String = value
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    mapped
        .trim_matches('-')
        .chars()
        .take(MAX_LABEL_LENGTH)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

pub async fn ensure_preview_domain(builds: &Builds, id: Uuid) -> Result<(), ServiceError> {
    let Some(domain) = builds.config.preview_domain.as_deref() else {
        return Ok(());
    };
    let deployment = builds.deployment(id).await?;
    let site = load(&builds.db, deployment.site_id).await?;
    let hostname = preview_hostname(
        deployment.pull_request,
        &deployment.branch,
        &site.name,
        domain,
    );
    let known = SiteDomains::find()
        .filter(site_domains::Column::Hostname.eq(hostname.as_str()))
        .one(&builds.db)
        .await?;
    if known.is_some() {
        return Ok(());
    }
    site_domains::ActiveModel {
        id: Set(Uuid::new_v4()),
        site_id: Set(site.id),
        hostname: Set(hostname),
        kind: Set("branch".to_string()),
        branch: Set(Some(deployment.branch)),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&builds.db)
    .await?;
    Ok(())
}

async fn start_runtime(
    builds: &Builds,
    site: &sites::Model,
    deployment: &site_deployments::Model,
    env: &BTreeMap<String, String>,
) -> Result<Uuid, ServiceError> {
    let image_ref = deployment
        .image_ref
        .as_deref()
        .ok_or_else(|| ServiceError::Invalid("deployment has no image".to_string()))?;
    let port = site
        .container_port
        .and_then(|port| u16::try_from(port).ok())
        .ok_or_else(|| ServiceError::Invalid("site has no container port".to_string()))?;
    let request = runtime_request(&RuntimeSpec {
        site_name: &site.name,
        deployment_id: deployment.id,
        resource_group_id: site.resource_group_id,
        image_ref,
        container_port: port,
        env,
    });
    builds.scheduler.create(&request).await
}

async fn current_workload(
    db: &DatabaseConnection,
    site: &sites::Model,
) -> Result<Option<Uuid>, ServiceError> {
    let Some(current) = site.current_deployment_id else {
        return Ok(None);
    };
    Ok(SiteDeployments::find_by_id(current)
        .one(db)
        .await?
        .and_then(|deployment| deployment.workload_id))
}

async fn mark_rolled_back(
    db: &DatabaseConnection,
    site: &sites::Model,
    promoted: &site_deployments::Model,
) -> Result<(), ServiceError> {
    let Some(current) = site.current_deployment_id else {
        return Ok(());
    };
    let Some(previous) = SiteDeployments::find_by_id(current).one(db).await? else {
        return Ok(());
    };
    if previous.id == promoted.id || previous.created_at <= promoted.created_at {
        return Ok(());
    }
    let mut active: site_deployments::ActiveModel = previous.into();
    active.status = Set("rolled_back".to_string());
    active.update(db).await?;
    Ok(())
}

pub async fn promote(
    builds: &Builds,
    id: Uuid,
    env: BTreeMap<String, String>,
) -> Result<(), ServiceError> {
    let deployment = builds.deployment(id).await?;
    if deployment.status != "ready" {
        return Err(ServiceError::Conflict("deployment is not ready"));
    }
    let site = load(&builds.db, deployment.site_id).await?;
    let mode = mode_of(&site)?;
    let previous_workload = current_workload(&builds.db, &site).await?;
    let workload = match mode {
        Mode::Container => Some(start_runtime(builds, &site, &deployment, &env).await?),
        Mode::Static => None,
    };
    mark_rolled_back(&builds.db, &site, &deployment).await?;
    let mut active: site_deployments::ActiveModel = deployment.into();
    active.environment = Set("production".to_string());
    active.workload_id = Set(workload);
    active.status = Set("ready".to_string());
    active.update(&builds.db).await?;
    let mut active_site: sites::ActiveModel = site.clone().into();
    active_site.current_deployment_id = Set(Some(id));
    active_site.update(&builds.db).await?;
    if mode == Mode::Container {
        match (previous_workload, workload) {
            (Some(old), Some(new)) => switch_over(builds, site.clone(), old, new),
            _ => routes::reconcile(&builds.db, &site, workload).await?,
        }
    }
    Ok(())
}

async fn release_workload(db: &DatabaseConnection, workload_id: Uuid) -> Result<(), DbErr> {
    SiteDeployments::update_many()
        .col_expr(
            site_deployments::Column::WorkloadId,
            Expr::value(None::<Uuid>),
        )
        .filter(site_deployments::Column::WorkloadId.eq(workload_id))
        .exec(db)
        .await?;
    Ok(())
}

fn switch_over(builds: &Builds, site: sites::Model, old: Uuid, new: Uuid) {
    let scheduler = builds.scheduler.clone();
    let db = builds.db.clone();
    tokio::spawn(async move {
        for _ in 0..RETIRE_ATTEMPTS {
            tokio::time::sleep(RETIRE_POLL).await;
            if !matches!(scheduler.status(new).await, Ok(Some(status)) if status == "running") {
                continue;
            }
            if let Err(error) = routes::reconcile(&db, &site, Some(new)).await {
                tracing::warn!(site = %site.id, error = %error, "route switch failed");
                return;
            }
            match scheduler.delete(old).await {
                Ok(()) => {
                    if let Err(error) = release_workload(&db, old).await {
                        tracing::warn!(workload = %old, error = %error, "workload release failed");
                    }
                }
                Err(error) => tracing::warn!(workload = %old, error = %error, "retire failed"),
            }
            return;
        }
        tracing::warn!(workload = %old, replacement = %new, "replacement never became running, keeping previous workload");
    });
}

pub async fn reconcile_site(builds: &Builds, id: Uuid) -> Result<(), ServiceError> {
    let site = load(&builds.db, id).await?;
    if mode_of(&site)? != Mode::Container {
        return Ok(());
    }
    let workload = current_workload(&builds.db, &site).await?;
    routes::reconcile(&builds.db, &site, workload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_hostnames_prefer_pull_request_numbers() {
        assert_eq!(
            preview_hostname(Some(48), "feature/coupons", "shop", "preview.csfx.app"),
            "pr-48-shop.preview.csfx.app"
        );
        assert_eq!(
            preview_hostname(None, "Feature/New_Search!", "shop", "preview.csfx.app"),
            "feature-new-search-shop.preview.csfx.app"
        );
    }

    #[test]
    fn slugs_are_trimmed_and_bounded() {
        assert_eq!(slug("--a--"), "a");
        assert_eq!(slug(&"x".repeat(100)).len(), MAX_LABEL_LENGTH);
    }
}
