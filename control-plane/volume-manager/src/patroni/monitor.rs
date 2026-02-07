use super::client::PatroniClient;
use super::types::*;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

/// Patroni Cluster Monitor
/// Überwacht kontinuierlich den PostgreSQL-Cluster Status
pub struct PatroniMonitor {
    client: Arc<PatroniClient>,
    check_interval: Duration,
}

impl PatroniMonitor {
    pub fn new(client: PatroniClient, check_interval_secs: u64) -> Self {
        Self {
            client: Arc::new(client),
            check_interval: Duration::from_secs(check_interval_secs),
        }
    }

    /// Startet den Monitoring-Loop (läuft in eigenem Task)
    pub async fn start_monitoring(self: Arc<Self>) {
        crate::log_info!("patroni_monitor", "Starting Patroni cluster monitoring");

        let mut check_interval = interval(self.check_interval);

        loop {
            check_interval.tick().await;

            match self.check_cluster_health().await {
                Ok(status) => {
                    self.log_cluster_status(&status);
                    
                    // Prüfe auf Probleme
                    if status.leader.is_none() {
                        crate::log_error!(
                            "patroni_monitor",
                            "⚠️  NO PRIMARY LEADER! Cluster in failover mode!"
                        );
                    }

                    let unhealthy_count = status
                        .members
                        .iter()
                        .filter(|m| m.state != PatroniState::Running)
                        .count();

                    if unhealthy_count > 0 {
                        crate::log_warn!(
                            "patroni_monitor",
                            &format!("⚠️  {} nodes unhealthy", unhealthy_count)
                        );
                    }
                }
                Err(e) => {
                    crate::log_error!(
                        "patroni_monitor",
                        &format!("Failed to check cluster health: {}", e)
                    );
                }
            }
        }
    }

    /// Prüft Cluster-Health
    async fn check_cluster_health(&self) -> Result<PatroniCluster> {
        self.client.get_cluster_status().await
    }

    /// Loggt Cluster-Status übersichtlich
    fn log_cluster_status(&self, cluster: &PatroniCluster) {
        crate::log_info!(
            "patroni_monitor",
            &format!(
                "Cluster '{}': Leader={:?}, Members={}",
                cluster.scope,
                cluster.leader,
                cluster.members.len()
            )
        );

        for member in &cluster.members {
            let role_icon = match member.role {
                PostgresNodeRole::Primary => "👑",
                PostgresNodeRole::Replica => "🔄",
                PostgresNodeRole::Standby => "⏸️",
                PostgresNodeRole::Unknown => "❓",
            };

            let state_icon = match member.state {
                PatroniState::Running => "✅",
                PatroniState::Starting => "🔄",
                PatroniState::Stopped => "⏹️",
                PatroniState::Failed => "❌",
                PatroniState::Unknown => "❓",
            };

            let lag_info = if let Some(lag) = member.lag {
                if lag > 1024 * 1024 {
                    // > 1MB lag
                    format!(" (LAG: {:.2}MB)", lag as f64 / 1024.0 / 1024.0)
                } else if lag > 0 {
                    format!(" (LAG: {}KB)", lag / 1024)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            crate::log_debug!(
                "patroni_monitor",
                &format!(
                    "  {} {} {:?} - {:?}{}",
                    role_icon, state_icon, member.name, member.state, lag_info
                )
            );
        }
    }

    /// Wartet bis Cluster bereit ist (Primary + mindestens 1 Replica)
    pub async fn wait_for_cluster_ready(&self, timeout_secs: u64) -> Result<()> {
        crate::log_info!(
            "patroni_monitor",
            &format!("Waiting for cluster to be ready (timeout: {}s)", timeout_secs)
        );

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for cluster to be ready");
            }

            match self.client.get_cluster_status().await {
                Ok(cluster) => {
                    let has_primary = cluster.leader.is_some();
                    let running_members = cluster
                        .members
                        .iter()
                        .filter(|m| m.state == PatroniState::Running)
                        .count();

                    if has_primary && running_members >= 2 {
                        crate::log_info!(
                            "patroni_monitor",
                            &format!(
                                "✅ Cluster ready! Primary={:?}, Running members={}",
                                cluster.leader, running_members
                            )
                        );
                        return Ok(());
                    }

                    crate::log_debug!(
                        "patroni_monitor",
                        &format!(
                            "Cluster not ready: Primary={}, Running={}",
                            has_primary, running_members
                        )
                    );
                }
                Err(e) => {
                    crate::log_debug!(
                        "patroni_monitor",
                        &format!("Cluster check failed: {}", e)
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Holt aktuelle Primary Node
    pub async fn get_primary(&self) -> Result<Option<PatroniNode>> {
        self.client.find_primary().await
    }

    /// Holt alle Replica Nodes
    pub async fn get_replicas(&self) -> Result<Vec<PatroniNode>> {
        self.client.find_replicas().await
    }

    /// Prüft ob Cluster healthy ist
    pub async fn is_cluster_healthy(&self) -> bool {
        match self.client.get_cluster_status().await {
            Ok(cluster) => {
                let has_primary = cluster.leader.is_some();
                let all_running = cluster
                    .members
                    .iter()
                    .all(|m| m.state == PatroniState::Running);

                has_primary && all_running
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitor_creation() {
        let client = PatroniClient::new(
            "test-scope".to_string(),
            vec![
                "http://patroni1:8008".to_string(),
                "http://patroni2:8008".to_string(),
            ],
        );

        let monitor = PatroniMonitor::new(client, 10);
        assert_eq!(monitor.check_interval, Duration::from_secs(10));
    }
}
