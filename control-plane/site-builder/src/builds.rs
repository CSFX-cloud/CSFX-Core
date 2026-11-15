use crate::config::Config;
use crate::detect::Mode;
use crate::error::ServiceError;
use crate::job::{builder_name, builder_request, stored_reference, BuildJob};
use crate::prune::{self, Candidate};
use crate::registry::client::RegistryClient;
use crate::scheduler::SchedulerClient;
use chrono::Utc;
use entity::{site_deployments, workloads, SiteArtifacts, SiteDeployments, Sites};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, Statement,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

const LOG_LIMIT_CHARS: i32 = 2_000_000;
const STALE_STATUSES: [&str; 2] = ["queued", "building"];

#[derive(Debug, Default, serde::Deserialize)]
pub struct Completion {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub commit_message: Option<String>,
    pub image_ref: Option<String>,
    pub error: Option<String>,
}

fn candidate_of(deployment: &site_deployments::Model) -> Candidate {
    Candidate {
        id: deployment.id,
        branch: deployment.branch.clone(),
        created_at: deployment.created_at,
        ready: deployment.status == "ready",
        bound_to_workload: deployment.workload_id.is_some(),
        has_payload: deployment.image_ref.is_some() || deployment.artifact_key.is_some(),
    }
}

fn build_env_of(
    deployment: &site_deployments::Model,
) -> Option<Result<BTreeMap<String, String>, ServiceError>> {
    let stored = deployment.build_env.clone()?;
    Some(serde_json::from_value(stored).map_err(|error| ServiceError::Invalid(error.to_string())))
}

fn trusted_image(
    mode: Mode,
    completion: &Completion,
    deployment: &site_deployments::Model,
) -> Result<Option<String>, ServiceError> {
    if mode == Mode::Static {
        return Ok(None);
    }
    let invalid = |message: &str| ServiceError::Invalid(message.to_string());
    if completion.image_ref.is_none() {
        return Err(invalid("build produced no image"));
    }
    if deployment.build_env.is_none() {
        return Err(invalid("build job state was lost"));
    }
    Ok(Some(stored_reference(deployment.site_id, deployment.id)))
}

pub struct Builds {
    pub db: DatabaseConnection,
    pub config: Arc<Config>,
    pub scheduler: SchedulerClient,
    registry: Option<RegistryClient>,
}

impl Builds {
    pub fn new(
        db: DatabaseConnection,
        config: Arc<Config>,
        scheduler: SchedulerClient,
        registry: Option<RegistryClient>,
    ) -> Self {
        Self {
            db,
            config,
            scheduler,
            registry,
        }
    }

    pub async fn deployment(&self, id: Uuid) -> Result<site_deployments::Model, ServiceError> {
        SiteDeployments::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or(ServiceError::NotFound)
    }

    pub async fn start(&self, job: BuildJob) -> Result<(), ServiceError> {
        let deployment = self.deployment(job.deployment_id).await?;
        if deployment.status != "queued" {
            return Err(ServiceError::Conflict("deployment is not queued"));
        }
        job.validate(&self.config)?;
        let request = builder_request(&self.config, &job);
        match self.scheduler.create(&request).await {
            Ok(workload_id) => self.mark_building(deployment, workload_id, job.env).await,
            Err(error) => {
                self.fail(deployment.id, &error.to_string()).await?;
                Err(error)
            }
        }
    }

    async fn mark_building(
        &self,
        deployment: site_deployments::Model,
        workload_id: Uuid,
        env: BTreeMap<String, String>,
    ) -> Result<(), ServiceError> {
        let id = deployment.id;
        let mut active: site_deployments::ActiveModel = deployment.into();
        active.status = Set("building".to_string());
        active.workload_id = Set(Some(workload_id));
        active.build_env = Set(Some(serde_json::json!(env)));
        active.update(&self.db).await?;
        tracing::info!(deployment = %id, workload = %workload_id, "build started");
        Ok(())
    }

    pub async fn append_log(&self, id: Uuid, text: &str) -> Result<(), ServiceError> {
        let statement = Statement::from_sql_and_values(
            self.db.get_database_backend(),
            "UPDATE site_deployments SET build_log = LEFT(build_log || $1, $2::integer) WHERE id = $3",
            [text.into(), LOG_LIMIT_CHARS.into(), id.into()],
        );
        self.db.execute(statement).await?;
        Ok(())
    }

    pub async fn fail(&self, id: Uuid, message: &str) -> Result<(), ServiceError> {
        let deployment = self.deployment(id).await?;
        if !STALE_STATUSES.contains(&deployment.status.as_str()) {
            return Ok(());
        }
        self.append_log(id, &format!("[ERROR] {message}\n")).await?;
        let builder = match deployment.workload_id {
            Some(workload_id) => Some(workload_id),
            None => self.unrecorded_builder(&deployment).await?,
        };
        let started = deployment.created_at;
        let mut active: site_deployments::ActiveModel = deployment.into();
        active.status = Set("failed".to_string());
        active.workload_id = Set(None);
        active.build_env = Set(None);
        let now = Utc::now().naive_utc();
        active.finished_at = Set(Some(now));
        active.duration_ms = Set(Some((now - started).num_milliseconds()));
        active.update(&self.db).await?;
        self.release_builder(builder).await;
        tracing::warn!(deployment = %id, reason = %message, "build failed");
        Ok(())
    }

    async fn unrecorded_builder(
        &self,
        deployment: &site_deployments::Model,
    ) -> Result<Option<Uuid>, ServiceError> {
        let Some(site) = Sites::find_by_id(deployment.site_id).one(&self.db).await? else {
            return Ok(None);
        };
        let builder = workloads::Entity::find()
            .filter(workloads::Column::ResourceGroupId.eq(site.resource_group_id))
            .filter(workloads::Column::Name.eq(builder_name(&site.name, deployment.id)))
            .one(&self.db)
            .await?;
        Ok(builder.map(|workload| workload.id))
    }

    pub async fn release_builder(&self, workload_id: Option<Uuid>) {
        let Some(workload_id) = workload_id else {
            return;
        };
        if let Err(error) = self.scheduler.delete(workload_id).await {
            tracing::warn!(workload = %workload_id, error = %error, "builder cleanup failed");
        }
    }

    pub async fn finish(&self, id: Uuid, completion: Completion) -> Result<(), ServiceError> {
        let deployment = self.deployment(id).await?;
        if deployment.status != "building" {
            return Err(ServiceError::Conflict("deployment is not building"));
        }
        if !completion.success {
            let reason = completion.error.as_deref().unwrap_or("build failed");
            return self.fail(id, reason).await;
        }
        match self.publish(deployment, completion).await {
            Ok(()) => Ok(()),
            Err(error) => {
                self.fail(id, &error.to_string()).await?;
                Err(error)
            }
        }
    }

    async fn publish(
        &self,
        deployment: site_deployments::Model,
        completion: Completion,
    ) -> Result<(), ServiceError> {
        let id = deployment.id;
        let site = crate::sites::load(&self.db, deployment.site_id).await?;
        let mode = crate::sites::mode_of(&site)?;
        if mode == Mode::Static && !crate::artifacts::exists(&self.db, id).await? {
            return Err(ServiceError::Invalid(
                "build produced no artifact".to_string(),
            ));
        }
        let image = trusted_image(mode, &completion, &deployment)?;
        let env = build_env_of(&deployment).transpose()?.unwrap_or_default();
        let builder = deployment.workload_id;
        let production = deployment.environment == "production";
        self.mark_ready(deployment, completion, mode, image).await?;
        self.release_builder(builder).await;
        if production {
            self.promote(id, env).await?;
        } else {
            crate::sites::ensure_preview_domain(self, id).await?;
        }
        self.prune_site(site.id).await;
        tracing::info!(deployment = %id, "deployment ready");
        Ok(())
    }

    async fn mark_ready(
        &self,
        deployment: site_deployments::Model,
        completion: Completion,
        mode: Mode,
        image: Option<String>,
    ) -> Result<(), ServiceError> {
        let started = deployment.created_at;
        let key = (mode == Mode::Static).then(|| deployment.id.to_string());
        let mut active: site_deployments::ActiveModel = deployment.into();
        let now = Utc::now().naive_utc();
        active.status = Set("ready".to_string());
        active.workload_id = Set(None);
        active.build_env = Set(None);
        active.artifact_key = Set(key);
        active.image_ref = Set(image);
        active.finished_at = Set(Some(now));
        active.duration_ms = Set(Some((now - started).num_milliseconds()));
        if let Some(sha) = completion.commit_sha {
            active.commit_sha = Set(sha);
        }
        if let Some(message) = completion.commit_message {
            active.commit_message = Set(message);
        }
        active.update(&self.db).await?;
        Ok(())
    }

    pub async fn promote(
        &self,
        id: Uuid,
        env: BTreeMap<String, String>,
    ) -> Result<(), ServiceError> {
        crate::sites::promote(self, id, env).await
    }

    pub fn registry(&self) -> Option<&RegistryClient> {
        self.registry.as_ref()
    }

    pub async fn prune_site(&self, site_id: Uuid) {
        if let Err(error) = self.prune(site_id).await {
            tracing::warn!(site = %site_id, error = %error, "pruning old deployments failed");
        }
    }

    async fn prune(&self, site_id: Uuid) -> Result<(), ServiceError> {
        let site = crate::sites::load(&self.db, site_id).await?;
        let deployments = SiteDeployments::find()
            .filter(site_deployments::Column::SiteId.eq(site_id))
            .all(&self.db)
            .await?;
        let candidates: Vec<Candidate> = deployments.iter().map(candidate_of).collect();
        let expired = prune::expired(
            &candidates,
            site.current_deployment_id,
            self.config.keep_deployments,
        );
        for deployment in deployments.into_iter().filter(|d| expired.contains(&d.id)) {
            let id = deployment.id;
            if let Err(error) = self.drop_payload(deployment).await {
                tracing::warn!(deployment = %id, error = %error, "could not remove old deployment data");
            }
        }
        Ok(())
    }

    async fn drop_payload(&self, deployment: site_deployments::Model) -> Result<(), ServiceError> {
        let id = deployment.id;
        let site_id = deployment.site_id;
        let image_tag = deployment
            .image_ref
            .as_deref()
            .and_then(prune::image_tag)
            .map(str::to_string);
        let mut active: site_deployments::ActiveModel = deployment.into();
        if let (Some(registry), Some(tag)) = (&self.registry, image_tag) {
            registry.delete_tag(&site_id.to_string(), &tag).await?;
        }
        SiteArtifacts::delete_by_id(id).exec(&self.db).await?;
        active.image_ref = Set(None);
        active.artifact_key = Set(None);
        active.update(&self.db).await?;
        tracing::info!(deployment = %id, "old deployment data removed");
        Ok(())
    }

    pub async fn purge_site(&self, site_id: Uuid) -> Result<(), ServiceError> {
        match &self.registry {
            Some(registry) => registry.delete_repository(&site_id.to_string()).await,
            None => Ok(()),
        }
    }

    pub async fn reap_stale(&self) -> Result<(), ServiceError> {
        let limit = chrono::Duration::from_std(self.config.build_timeout)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        let cutoff = Utc::now().naive_utc() - limit;
        let stale = SiteDeployments::find()
            .filter(site_deployments::Column::Status.is_in(STALE_STATUSES))
            .filter(site_deployments::Column::CreatedAt.lt(cutoff))
            .all(&self.db)
            .await?;
        for deployment in stale {
            self.fail(deployment.id, "build timed out").await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn deployment(build_env: Option<serde_json::Value>) -> site_deployments::Model {
        site_deployments::Model {
            id: Uuid::from_u128(1),
            site_id: Uuid::nil(),
            commit_sha: String::new(),
            commit_message: String::new(),
            branch: "main".to_string(),
            pull_request: None,
            environment: "production".to_string(),
            status: "building".to_string(),
            artifact_key: None,
            image_ref: None,
            workload_id: None,
            build_log: String::new(),
            build_env,
            duration_ms: None,
            created_at: Utc::now().naive_utc(),
            finished_at: None,
        }
    }

    fn started() -> site_deployments::Model {
        deployment(Some(serde_json::json!({})))
    }

    fn reported(image: Option<&str>) -> Completion {
        Completion {
            success: true,
            image_ref: image.map(str::to_string),
            ..Completion::default()
        }
    }

    #[test]
    fn container_images_ignore_the_value_reported_by_the_vm() {
        let image = trusted_image(Mode::Container, &reported(Some("evil/other:1")), &started())
            .unwrap()
            .unwrap();
        assert_eq!(
            image,
            format!("sites.csfx.invalid/{}:00000000", Uuid::nil())
        );
    }

    #[test]
    fn container_images_need_a_report_and_a_job() {
        assert!(trusted_image(Mode::Container, &reported(None), &started()).is_err());
        assert!(trusted_image(Mode::Container, &reported(Some("x")), &deployment(None)).is_err());
    }

    #[test]
    fn persisted_env_survives_a_restart() {
        let env = BTreeMap::from([("API_KEY".to_string(), "secret".to_string())]);
        let stored = deployment(Some(serde_json::json!(env)));
        assert_eq!(build_env_of(&stored).unwrap().unwrap(), env);
        assert!(build_env_of(&deployment(None)).is_none());
    }

    #[test]
    fn static_builds_have_no_image() {
        assert_eq!(
            trusted_image(Mode::Static, &reported(None), &deployment(None)).unwrap(),
            None
        );
    }
}
