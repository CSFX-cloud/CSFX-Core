use crate::error::ServiceError;
use chrono::Utc;
use entity::{ingress_routes, site_deployments, site_domains, SiteDeployments, SiteDomains};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;
use uuid::Uuid;

async fn site_workloads(db: &DatabaseConnection, site_id: Uuid) -> Result<Vec<Uuid>, ServiceError> {
    Ok(SiteDeployments::find()
        .filter(site_deployments::Column::SiteId.eq(site_id))
        .filter(site_deployments::Column::WorkloadId.is_not_null())
        .all(db)
        .await?
        .into_iter()
        .filter_map(|deployment| deployment.workload_id)
        .collect())
}

async fn desired_hostnames(
    db: &DatabaseConnection,
    site_id: Uuid,
) -> Result<HashSet<String>, ServiceError> {
    Ok(SiteDomains::find()
        .filter(site_domains::Column::SiteId.eq(site_id))
        .filter(site_domains::Column::Kind.is_in(["production", "auto"]))
        .all(db)
        .await?
        .into_iter()
        .map(|domain| domain.hostname)
        .collect())
}

async fn drop_stale(
    db: &DatabaseConnection,
    existing: Vec<ingress_routes::Model>,
    wanted: &HashSet<String>,
    workload_id: Uuid,
) -> Result<HashSet<String>, ServiceError> {
    let mut kept = HashSet::new();
    for route in existing {
        let current = route.workload_id == workload_id && wanted.contains(&route.hostname);
        if current {
            kept.insert(route.hostname);
            continue;
        }
        ingress_routes::Entity::delete_by_id(route.id)
            .exec(db)
            .await?;
    }
    Ok(kept)
}

pub async fn reconcile(
    db: &DatabaseConnection,
    site: &entity::sites::Model,
    workload_id: Option<Uuid>,
) -> Result<(), ServiceError> {
    let owned = site_workloads(db, site.id).await?;
    let existing = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::WorkloadId.is_in(owned))
        .all(db)
        .await?;
    let wanted = match workload_id {
        Some(_) => desired_hostnames(db, site.id).await?,
        None => HashSet::new(),
    };
    let kept = drop_stale(db, existing, &wanted, workload_id.unwrap_or_default()).await?;
    let (Some(workload_id), Some(port)) = (workload_id, site.container_port) else {
        return Ok(());
    };
    for hostname in wanted.difference(&kept) {
        ingress_routes::ActiveModel {
            id: Set(Uuid::new_v4()),
            resource_group_id: Set(site.resource_group_id),
            workload_id: Set(workload_id),
            gateway_id: Set(site.gateway_id),
            hostname: Set(hostname.clone()),
            container_port: Set(port),
            tls: Set(true),
            created_at: Set(Utc::now().naive_utc()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}
