use anyhow::{Context, Result};
use entity::entities::workloads;
use reqwest::Client;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use shared::qos::QosClass;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

const COOLDOWN: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressure {
    Cpu,
    Memory,
}

pub struct EvictionService {
    db: DatabaseConnection,
    http: Client,
    scheduler_url: String,
    last_eviction: Mutex<HashMap<Uuid, Instant>>,
}

impl EvictionService {
    pub fn new(db: DatabaseConnection) -> Self {
        let scheduler_url = std::env::var("SCHEDULER_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8002".to_string());

        Self {
            db,
            http: Client::new(),
            scheduler_url,
            last_eviction: Mutex::default(),
        }
    }

    pub async fn relieve(&self, agent_id: Uuid, pressure: Pressure) -> Result<()> {
        if self.in_cooldown(agent_id).await {
            return Ok(());
        }

        let running = self.running_workloads(agent_id).await?;
        let Some(victim) = pick_victim(&running, pressure) else {
            return Ok(());
        };

        self.last_eviction
            .lock()
            .await
            .insert(agent_id, Instant::now());

        let moved = self.request_move(agent_id, victim).await?;
        crate::log_warn!(
            "eviction",
            &format!(
                "Eviction requested agent_id={} workload_id={} pressure={:?} moved={}",
                agent_id, victim, pressure, moved
            )
        );

        if moved {
            crate::db::events::insert(
                &self.db,
                Some(agent_id),
                "eviction",
                Some(vec![victim]),
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn in_cooldown(&self, agent_id: Uuid) -> bool {
        self.last_eviction
            .lock()
            .await
            .get(&agent_id)
            .is_some_and(|last| last.elapsed() < COOLDOWN)
    }

    async fn running_workloads(&self, agent_id: Uuid) -> Result<Vec<workloads::Model>> {
        Ok(workloads::Entity::find()
            .filter(workloads::Column::AssignedAgentId.eq(agent_id))
            .filter(workloads::Column::Status.is_in(shared::workload_status::ACTIVE_STATUSES))
            .all(&self.db)
            .await?)
    }

    async fn request_move(&self, agent_id: Uuid, workload_id: Uuid) -> Result<bool> {
        let url = format!(
            "{}/internal/agents/{}/reschedule",
            self.scheduler_url, agent_id
        );

        let response = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "workload_ids": [workload_id] }))
            .send()
            .await
            .with_context(|| format!("eviction request failed agent_id={}", agent_id))?
            .error_for_status()
            .with_context(|| format!("eviction request rejected agent_id={}", agent_id))?;

        let results: Vec<serde_json::Value> = response.json().await?;
        Ok(results
            .first()
            .and_then(|result| result.get("status"))
            .is_some_and(|status| status == "scheduled"))
    }
}

fn has_volumes(workload: &workloads::Model) -> bool {
    workload
        .volume_mounts
        .as_ref()
        .and_then(|mounts| mounts.as_array())
        .is_some_and(|mounts| !mounts.is_empty())
}

fn usage(workload: &workloads::Model, pressure: Pressure) -> f64 {
    match pressure {
        Pressure::Cpu => workload.cpu_usage_percent.unwrap_or(0.0),
        Pressure::Memory => workload.memory_usage_bytes.unwrap_or(0) as f64,
    }
}

pub fn pick_victim(candidates: &[workloads::Model], pressure: Pressure) -> Option<Uuid> {
    candidates
        .iter()
        .filter(|workload| workload.status == "running")
        .filter(|workload| !has_volumes(workload))
        .map(|workload| (qos_of(workload), usage(workload, pressure), workload.id))
        .filter(|(qos, _, _)| *qos != QosClass::Guaranteed)
        .min_by(|left, right| left.0.cmp(&right.0).then(right.1.total_cmp(&left.1)))
        .map(|(_, _, id)| id)
}

fn qos_of(workload: &workloads::Model) -> QosClass {
    QosClass::classify(
        workload.cpu_millicores,
        workload.cpu_limit_millicores,
        workload.memory_bytes,
        workload.memory_limit_bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workload(
        cpu: i32,
        cpu_limit: Option<i32>,
        memory_usage: i64,
        volumes: bool,
    ) -> workloads::Model {
        workloads::Model {
            id: Uuid::new_v4(),
            name: "w".to_string(),
            image: "i".to_string(),
            cpu_millicores: cpu,
            memory_bytes: 1024,
            disk_bytes: 0,
            cpu_limit_millicores: cpu_limit,
            memory_limit_bytes: None,
            spread_key: None,
            guest_ip: None,
            auto_resources: false,
            cpu_usage_peak_percent: None,
            memory_usage_peak_bytes: None,
            env_vars: None,
            ports: None,
            volume_mounts: volumes.then(|| serde_json::json!([{ "volume_id": "v" }])),
            status: "running".to_string(),
            assigned_agent_id: None,
            container_id: None,
            created_by: None,
            organization_id: None,
            resource_group_id: None,
            stack_id: None,
            service_name: None,
            restart_policy: "always".to_string(),
            max_restarts: None,
            restart_count: 0,
            runtime_class: "firecracker".to_string(),
            desired_state: "running".to_string(),
            restart_requested: false,
            cpu_usage_percent: None,
            memory_usage_bytes: Some(memory_usage),
            network_rx_bytes: None,
            network_tx_bytes: None,
            stats_updated_at: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn never_evicts_guaranteed_or_volume_backed_workloads() {
        let candidates = vec![
            workload(500, None, 900, false),
            workload(500, Some(1000), 100, true),
        ];
        assert_eq!(pick_victim(&candidates, Pressure::Memory), None);
    }

    #[test]
    fn prefers_besteffort_over_burstable() {
        let burstable = workload(500, Some(1000), 900, false);
        let mut besteffort = workload(0, None, 10, false);
        besteffort.memory_bytes = 0;
        let expected = besteffort.id;
        let candidates = vec![burstable, besteffort];
        assert_eq!(pick_victim(&candidates, Pressure::Memory), Some(expected));
    }

    #[test]
    fn picks_heaviest_user_within_the_same_class() {
        let light = workload(500, Some(1000), 100, false);
        let heavy = workload(500, Some(1000), 900, false);
        let expected = heavy.id;
        let candidates = vec![light, heavy];
        assert_eq!(pick_victim(&candidates, Pressure::Memory), Some(expected));
    }
}
