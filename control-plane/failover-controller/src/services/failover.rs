use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

pub struct FailoverService {
    db: DatabaseConnection,
    http: Client,
    scheduler_url: String,
    volume_manager_url: String,
}

impl FailoverService {
    pub fn new(db: DatabaseConnection) -> Self {
        let scheduler_url = std::env::var("SCHEDULER_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8002".to_string());
        let volume_manager_url = std::env::var("VOLUME_MANAGER_URL")
            .unwrap_or_else(|_| "http://localhost:8003".to_string());

        Self {
            db,
            http: Client::new(),
            scheduler_url,
            volume_manager_url,
        }
    }

    pub async fn handle_failover(&self, agent_id: Uuid) -> Result<()> {
        let started_at = Utc::now();

        crate::log_info!(
            "failover",
            &format!("Failover sequence started agent_id={}", agent_id)
        );

        let workload_ids = self.get_agent_workload_ids(agent_id).await?;

        self.force_detach_volumes(agent_id).await;
        self.reschedule_workloads(agent_id, &workload_ids).await;

        let duration_ms = (Utc::now() - started_at).num_milliseconds();

        crate::db::events::insert(
            &self.db,
            Some(agent_id),
            "failover_complete",
            Some(workload_ids.clone()),
            Some(duration_ms),
        )
        .await?;

        crate::log_info!(
            "failover",
            &format!(
                "Failover complete agent_id={} workloads={} duration_ms={}",
                agent_id,
                workload_ids.len(),
                duration_ms
            )
        );

        Ok(())
    }

    async fn get_agent_workload_ids(&self, agent_id: Uuid) -> Result<Vec<Uuid>> {
        use entity::entities::workloads;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let rows = workloads::Entity::find()
            .filter(workloads::Column::AssignedAgentId.eq(agent_id))
            .filter(
                workloads::Column::Status
                    .eq("scheduled")
                    .or(workloads::Column::Status.eq("running")),
            )
            .all(&self.db)
            .await?;

        Ok(rows.into_iter().map(|w| w.id).collect())
    }

    async fn force_detach_volumes(&self, agent_id: Uuid) {
        let url = format!(
            "{}/internal/agents/{}/detach-all",
            self.volume_manager_url, agent_id
        );

        match self.http.post(&url).send().await {
            Ok(resp) => {
                crate::log_info!(
                    "failover",
                    &format!(
                        "Volume detach request sent agent_id={} status={}",
                        agent_id,
                        resp.status()
                    )
                );
            }
            Err(e) => {
                crate::log_warn!(
                    "failover",
                    &format!(
                        "Volume detach request failed agent_id={} err={}",
                        agent_id, e
                    )
                );
            }
        }
    }

    async fn reschedule_workloads(&self, agent_id: Uuid, workload_ids: &[Uuid]) {
        let url = format!(
            "{}/internal/agents/{}/reschedule",
            self.scheduler_url, agent_id
        );

        let body = serde_json::json!({ "workload_ids": workload_ids });

        match self.http.post(&url).json(&body).send().await {
            Ok(resp) => {
                crate::log_info!(
                    "failover",
                    &format!(
                        "Reschedule request sent agent_id={} count={} status={}",
                        agent_id,
                        workload_ids.len(),
                        resp.status()
                    )
                );
            }
            Err(e) => {
                crate::log_warn!(
                    "failover",
                    &format!(
                        "Reschedule request failed agent_id={} err={}",
                        agent_id, e
                    )
                );
            }
        }
    }
}
