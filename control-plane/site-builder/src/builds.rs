use crate::config::Config;
use crate::error::ServiceError;
use crate::job::{builder_request, image_reference, BuildJob};
use crate::scheduler::SchedulerClient;
use chrono::Utc;
use entity::{site_deployments, SiteDeployments};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, Statement,
};
use site_builder::detect::Mode;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const LOG_LIMIT_CHARS: i64 = 2_000_000;
const STALE_STATUSES: [&str; 2] = ["queued", "building"];

#[derive(Debug, Default, serde::Deserialize)]
pub struct Completion {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub commit_message: Option<String>,
    pub image_ref: Option<String>,
    pub error: Option<String>,
}

fn trusted_image(
    registry: Option<&str>,
    mode: Mode,
    completion: &Completion,
    job: Option<&BuildJob>,
) -> Result<Option<String>, ServiceError> {
    if mode == Mode::Static {
        return Ok(None);
    }
    let invalid = |message: &str| ServiceError::Invalid(message.to_string());
    if completion.image_ref.is_none() {
        return Err(invalid("build produced no image"));
    }
    let job = job.ok_or_else(|| invalid("build job state was lost"))?;
    let registry = registry.ok_or_else(|| invalid("no site registry is configured"))?;
    Ok(Some(image_reference(registry, job)))
}

pub struct Builds {
    pub db: DatabaseConnection,
    pub config: Arc<Config>,
    pub scheduler: SchedulerClient,
    pending: Mutex<HashMap<Uuid, BuildJob>>,
}

impl Builds {
    pub fn new(db: DatabaseConnection, config: Arc<Config>, scheduler: SchedulerClient) -> Self {
        Self {
            db,
            config,
            scheduler,
            pending: Mutex::new(HashMap::new()),
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
            Ok(workload_id) => self.mark_building(deployment, workload_id, job).await,
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
        job: BuildJob,
    ) -> Result<(), ServiceError> {
        let id = deployment.id;
        let mut active: site_deployments::ActiveModel = deployment.into();
        active.status = Set("building".to_string());
        active.workload_id = Set(Some(workload_id));
        active.update(&self.db).await?;
        self.remember(id, job);
        tracing::info!(deployment = %id, workload = %workload_id, "build started");
        Ok(())
    }

    fn remember(&self, id: Uuid, job: BuildJob) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, job);
    }

    pub fn recall(&self, id: Uuid) -> Option<BuildJob> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&id)
    }

    pub async fn append_log(&self, id: Uuid, text: &str) -> Result<(), ServiceError> {
        let statement = Statement::from_sql_and_values(
            self.db.get_database_backend(),
            "UPDATE site_deployments SET build_log = LEFT(build_log || $1, $2) WHERE id = $3",
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
        let builder = deployment.workload_id;
        let started = deployment.created_at;
        let mut active: site_deployments::ActiveModel = deployment.into();
        active.status = Set("failed".to_string());
        active.workload_id = Set(None);
        let now = Utc::now().naive_utc();
        active.finished_at = Set(Some(now));
        active.duration_ms = Set(Some((now - started).num_milliseconds()));
        active.update(&self.db).await?;
        self.recall(id);
        self.release_builder(builder).await;
        tracing::warn!(deployment = %id, reason = %message, "build failed");
        Ok(())
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
        let job = self.recall(id);
        if mode == Mode::Static && !crate::artifacts::exists(&self.db, id).await? {
            return Err(ServiceError::Invalid(
                "build produced no artifact".to_string(),
            ));
        }
        let image = trusted_image(
            self.config.registry.as_deref(),
            mode,
            &completion,
            job.as_ref(),
        )?;
        let builder = deployment.workload_id;
        let production = deployment.environment == "production";
        self.mark_ready(deployment, completion, mode, image).await?;
        self.release_builder(builder).await;
        if production {
            let env = job.map(|job| job.env).unwrap_or_default();
            self.promote(id, env).await?;
        } else {
            crate::sites::ensure_preview_domain(self, id).await?;
        }
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

    fn job() -> BuildJob {
        BuildJob {
            deployment_id: Uuid::from_u128(1),
            site_id: Uuid::nil(),
            resource_group_id: Uuid::nil(),
            site_name: "shop".to_string(),
            mode: Mode::Container,
            repo_url: "https://example.com/a".to_string(),
            branch: "main".to_string(),
            root_dir: String::new(),
            framework: None,
            install_command: None,
            build_command: None,
            output_dir: None,
            start_command: None,
            container_port: Some(3000),
            git_token: None,
            env: BTreeMap::new(),
        }
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
        let image = trusted_image(
            Some("reg:5060"),
            Mode::Container,
            &reported(Some("evil/other:1")),
            Some(&job()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(image, format!("reg:5060/{}:00000000", Uuid::nil()));
    }

    #[test]
    fn container_images_need_a_report_a_job_and_a_registry() {
        let job = job();
        assert!(trusted_image(Some("r"), Mode::Container, &reported(None), Some(&job)).is_err());
        assert!(trusted_image(Some("r"), Mode::Container, &reported(Some("x")), None).is_err());
        assert!(trusted_image(None, Mode::Container, &reported(Some("x")), Some(&job)).is_err());
    }

    #[test]
    fn static_builds_have_no_image() {
        assert_eq!(
            trusted_image(None, Mode::Static, &reported(None), None).unwrap(),
            None
        );
    }
}
