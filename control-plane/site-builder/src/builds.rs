use crate::config::Config;
use crate::error::ServiceError;
use crate::job::{builder_request, BuildJob};
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
        if mode == Mode::Static && !crate::artifacts::exists(&self.db, id).await? {
            return Err(ServiceError::Invalid(
                "build produced no artifact".to_string(),
            ));
        }
        if mode == Mode::Container && completion.image_ref.is_none() {
            return Err(ServiceError::Invalid("build produced no image".to_string()));
        }
        let builder = deployment.workload_id;
        let production = deployment.environment == "production";
        self.mark_ready(deployment, completion, mode).await?;
        self.release_builder(builder).await;
        let env = self.recall(id).map(|job| job.env).unwrap_or_default();
        if production {
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
    ) -> Result<(), ServiceError> {
        let started = deployment.created_at;
        let key = (mode == Mode::Static).then(|| deployment.id.to_string());
        let mut active: site_deployments::ActiveModel = deployment.into();
        let now = Utc::now().naive_utc();
        active.status = Set("ready".to_string());
        active.workload_id = Set(None);
        active.artifact_key = Set(key);
        active.image_ref = Set(completion.image_ref);
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
