use anyhow::Result;
use chrono::Utc;
use entity::entities::{agent_metrics, agents};
use sea_orm::{
    prelude::Json, ActiveModelTrait, ActiveValue, ColumnTrait, DbConn, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sysinfo::{Disks, Networks, System};
use tokio::time::{interval, Duration};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSystemMetrics {
    pub agent_id: Uuid,
    pub timestamp: chrono::DateTime<Utc>,

    // CPU
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub cpu_usage_percent: f32,

    // Memory
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_usage_percent: f32,

    // Disk
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_usage_percent: f32,

    // Network
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,

    // System
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub hostname: String,
    pub uptime_seconds: u64,
}

pub struct SelfMonitor {
    agent_id: Uuid,
    db_conn: Arc<DbConn>,
    system: System,
    networks: Networks,
    disks: Disks,
}

impl SelfMonitor {
    pub async fn new(db_conn: Arc<DbConn>) -> Result<Self> {
        // Get or create local agent
        let hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());
        let agent_name = format!("CSF-Core-{}", hostname);

        // Check if agent already exists
        let existing_agent = agents::Entity::find()
            .filter(agents::Column::Hostname.eq(&hostname))
            .filter(agents::Column::Name.eq(&agent_name))
            .one(db_conn.as_ref())
            .await?;

        let agent_id = if let Some(agent) = existing_agent {
            tracing::info!("🔄 Using existing local agent: {}", agent.id);
            agent.id
        } else {
            // Create new agent
            let new_agent = agents::ActiveModel {
                id: ActiveValue::Set(Uuid::new_v4()),
                name: ActiveValue::Set(agent_name.clone()),
                hostname: ActiveValue::Set(hostname.clone()),
                ip_address: ActiveValue::Set(Some("127.0.0.1".to_string())),
                agent_version: ActiveValue::Set(env!("CARGO_PKG_VERSION").to_string()),
                os_type: ActiveValue::Set(System::name().unwrap_or_else(|| "Unknown".to_string())),
                os_version: ActiveValue::Set(
                    System::os_version().unwrap_or_else(|| "Unknown".to_string()),
                ),
                architecture: ActiveValue::Set(std::env::consts::ARCH.to_string()),
                status: ActiveValue::Set("online".to_string()),
                last_heartbeat: ActiveValue::Set(Some(Utc::now().naive_utc())),
                registered_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(None),
                organization_id: ActiveValue::Set(None),
                tags: ActiveValue::Set(None),
                capabilities: ActiveValue::Set(Some(Json::Array(vec![Json::String(
                    "self-monitor".to_string(),
                )]))),
            };

            let agent = new_agent.insert(db_conn.as_ref()).await?;
            tracing::info!("✅ Registered local agent: {} ({})", agent_name, agent.id);
            agent.id
        };

        Ok(Self {
            agent_id,
            db_conn,
            system: System::new_all(),
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
        })
    }

    pub fn collect_metrics(&mut self) -> LocalSystemMetrics {
        // Refresh all data
        self.system.refresh_all();
        self.networks.refresh();
        self.disks.refresh();

        // CPU info
        let cpu_model = self
            .system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let cpu_cores = self.system.physical_core_count().unwrap_or(0) as u32;
        let cpu_threads = self.system.cpus().len() as u32;

        let cpu_usage_percent = if !self.system.cpus().is_empty() {
            let total: f32 = self.system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum();
            total / self.system.cpus().len() as f32
        } else {
            0.0
        };

        // Memory
        let memory_total_bytes = self.system.total_memory();
        let memory_used_bytes = self.system.used_memory();

        let memory_usage_percent = if memory_total_bytes > 0 {
            (memory_used_bytes as f32 / memory_total_bytes as f32) * 100.0
        } else {
            0.0
        };

        // Disk
        let (disk_total_bytes, disk_used_bytes) =
            self.disks.iter().fold((0u64, 0u64), |(total, used), disk| {
                (
                    total + disk.total_space(),
                    used + (disk.total_space() - disk.available_space()),
                )
            });

        let disk_usage_percent = if disk_total_bytes > 0 {
            (disk_used_bytes as f32 / disk_total_bytes as f32) * 100.0
        } else {
            0.0
        };

        // Network
        let (network_rx_bytes, network_tx_bytes) =
            self.networks
                .iter()
                .fold((0u64, 0u64), |(rx, tx), (_name, network)| {
                    (
                        rx + network.total_received(),
                        tx + network.total_transmitted(),
                    )
                });

        // System info
        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let uptime_seconds = System::uptime();

        LocalSystemMetrics {
            agent_id: self.agent_id,
            timestamp: Utc::now(),
            cpu_model,
            cpu_cores,
            cpu_threads,
            cpu_usage_percent,
            memory_total_bytes,
            memory_used_bytes,
            memory_usage_percent,
            disk_total_bytes,
            disk_used_bytes,
            disk_usage_percent,
            network_rx_bytes,
            network_tx_bytes,
            os_name,
            os_version,
            kernel_version,
            hostname,
            uptime_seconds,
        }
    }

    pub async fn store_metrics(&self, metrics: &LocalSystemMetrics) -> Result<()> {
        // Store metrics in database
        let new_metrics = agent_metrics::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            agent_id: ActiveValue::Set(metrics.agent_id),
            timestamp: ActiveValue::Set(metrics.timestamp.naive_utc()),
            cpu_model: ActiveValue::Set(Some(metrics.cpu_model.clone())),
            cpu_cores: ActiveValue::Set(Some(metrics.cpu_cores as i32)),
            cpu_threads: ActiveValue::Set(Some(metrics.cpu_threads as i32)),
            cpu_usage_percent: ActiveValue::Set(Some(metrics.cpu_usage_percent)),
            memory_total_bytes: ActiveValue::Set(Some(metrics.memory_total_bytes as i64)),
            memory_used_bytes: ActiveValue::Set(Some(metrics.memory_used_bytes as i64)),
            memory_usage_percent: ActiveValue::Set(Some(metrics.memory_usage_percent)),
            disk_total_bytes: ActiveValue::Set(Some(metrics.disk_total_bytes as i64)),
            disk_used_bytes: ActiveValue::Set(Some(metrics.disk_used_bytes as i64)),
            disk_usage_percent: ActiveValue::Set(Some(metrics.disk_usage_percent)),
            network_rx_bytes: ActiveValue::Set(Some(metrics.network_rx_bytes as i64)),
            network_tx_bytes: ActiveValue::Set(Some(metrics.network_tx_bytes as i64)),
            os_name: ActiveValue::Set(Some(metrics.os_name.clone())),
            os_version: ActiveValue::Set(Some(metrics.os_version.clone())),
            kernel_version: ActiveValue::Set(Some(metrics.kernel_version.clone())),
            hostname: ActiveValue::Set(Some(metrics.hostname.clone())),
            uptime_seconds: ActiveValue::Set(Some(metrics.uptime_seconds as i64)),
            custom_metrics: ActiveValue::Set(None),
        };

        new_metrics.insert(self.db_conn.as_ref()).await?;
        Ok(())
    }

    pub async fn update_agent_status(&self) -> Result<()> {
        // Update agent heartbeat
        if let Some(agent) = agents::Entity::find_by_id(self.agent_id)
            .one(self.db_conn.as_ref())
            .await?
        {
            let mut agent_active: agents::ActiveModel = agent.into();
            agent_active.last_heartbeat = ActiveValue::Set(Some(Utc::now().naive_utc()));
            agent_active.status = ActiveValue::Set("online".to_string());
            agent_active.update(self.db_conn.as_ref()).await?;
        }
        Ok(())
    }

    pub async fn run(mut self) {
        tracing::info!(
            "🚀 Self-monitoring service started for agent {}",
            self.agent_id
        );

        let mut metrics_interval = interval(Duration::from_secs(60)); // Every 60 seconds
        let mut heartbeat_interval = interval(Duration::from_secs(30)); // Every 30 seconds

        loop {
            tokio::select! {
                _ = metrics_interval.tick() => {
                    let metrics = self.collect_metrics();

                    tracing::info!(
                        "📊 Local Metrics - CPU: {:.1}% | RAM: {:.1}% | Disk: {:.1}%",
                        metrics.cpu_usage_percent,
                        metrics.memory_usage_percent,
                        metrics.disk_usage_percent
                    );

                    if let Err(e) = self.store_metrics(&metrics).await {
                        tracing::error!("Failed to store metrics: {}", e);
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if let Err(e) = self.update_agent_status().await {
                        tracing::error!("Failed to update agent status: {}", e);
                    }
                }
            }
        }
    }
}

pub async fn start_self_monitoring(db_conn: Arc<DbConn>) {
    match SelfMonitor::new(db_conn).await {
        Ok(monitor) => {
            tokio::spawn(async move {
                let _ = monitor.run().await;
            });
        }
        Err(e) => {
            tracing::error!("Failed to start self-monitoring service: {}", e);
        }
    }
}
