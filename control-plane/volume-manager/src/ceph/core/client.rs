use super::config::CephConfig;
use crate::ceph::storage::types::*;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use tokio::process::Command as AsyncCommand;

#[derive(Clone)]
pub struct CephClient {
    config: CephConfig,
}

impl CephClient {
    pub fn new(config: CephConfig) -> Self {
        Self { config }
    }

    pub async fn execute(&self, cmd: CephCommand) -> Result<String> {
        self.run("ceph", cmd, true).await
    }

    pub async fn execute_rbd(&self, cmd: CephCommand) -> Result<String> {
        self.run("rbd", cmd, false).await
    }

    async fn run(&self, program: &str, cmd: CephCommand, json_output: bool) -> Result<String> {
        let cmd_vec = cmd.to_vec();
        crate::log_debug!(
            "ceph_client",
            &format!("Executing {} command: {}", program, cmd_vec.join(" "))
        );

        let mut command = AsyncCommand::new(program);
        command.arg("-m").arg(self.config.mon_host_string());
        if let Some(ref keyring) = self.config.keyring_path {
            command.arg("--keyring").arg(keyring);
        }
        command
            .arg("--name")
            .arg(format!("client.{}", self.config.client_name));
        command.args(cmd_vec);
        if json_output {
            command.arg("--format").arg("json");
        }

        let output = command
            .output()
            .await
            .with_context(|| format!("Failed to execute {} command", program))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            crate::log_error!(
                "ceph_client",
                &format!("{} command failed: {}", program, stderr)
            );
            return Err(anyhow!("{} command failed: {}", program, stderr));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    /// Prüft Cluster-Health
    pub async fn health_status(&self) -> Result<CephClusterHealth> {
        crate::log_debug!("ceph_client", "Checking cluster health status");

        let cmd = CephCommand::new("status");
        let output = self.execute(cmd).await?;

        // Parse JSON output
        let status: Value = serde_json::from_str(&output)?;

        // Extrahiere Health-Status
        let health_status = status["health"]["status"].as_str().unwrap_or("HEALTH_ERR");

        let health = match health_status {
            "HEALTH_OK" => HealthStatus::Ok,
            "HEALTH_WARN" => HealthStatus::Warn,
            _ => HealthStatus::Error,
        };

        crate::log_debug!(
            "ceph_client",
            &format!("Cluster health status: {:?}", health)
        );

        // Extrahiere Monitor-Info
        let mons = if let Some(mon_map) = status["monmap"]["mons"].as_array() {
            mon_map
                .iter()
                .filter_map(|m| {
                    Some(MonitorInfo {
                        name: m["name"].as_str()?.to_string(),
                        addr: m["addr"].as_str()?.to_string(),
                        rank: m["rank"].as_u64()? as u32,
                        in_quorum: true, // Simplified
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        // Extrahiere OSD-Info
        let osds = if let Some(osd_map) = status["osdmap"]["osds"].as_array() {
            osd_map
                .iter()
                .filter_map(|o| {
                    Some(OsdInfo {
                        id: o["osd"].as_u64()? as u32,
                        up: o["up"].as_u64()? == 1,
                        in_cluster: o["in"].as_u64()? == 1,
                        weight: o["weight"].as_f64()?,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        // PG Summary
        let pgs = PgSummary {
            total: status["pgmap"]["num_pgs"].as_u64().unwrap_or(0) as u32,
            active_clean: status["pgmap"]["pgs_by_state"]
                .as_array()
                .and_then(|arr| {
                    arr.iter()
                        .find(|s| s["state_name"].as_str() == Some("active+clean"))
                        .and_then(|s| s["count"].as_u64())
                })
                .unwrap_or(0) as u32,
            degraded: 0,  // Simplified
            misplaced: 0, // Simplified
        };

        Ok(CephClusterHealth {
            status: health,
            mons,
            osds,
            pgs,
        })
    }

    pub async fn blocklist_add(&self, address: &str) -> Result<()> {
        let cmd = CephCommand::new("osd")
            .arg("blocklist")
            .arg("add")
            .arg(address);
        self.execute(cmd)
            .await
            .with_context(|| format!("Failed to blocklist client address={}", address))?;
        Ok(())
    }

    /// Prüft ob Ceph-Cluster erreichbar ist
    pub async fn is_available(&self) -> bool {
        self.health_status().await.is_ok()
    }

    /// Wartet bis Cluster verfügbar ist
    pub async fn wait_for_cluster(&self, max_attempts: u32) -> Result<()> {
        crate::log_info!(
            "ceph_client",
            &format!("Waiting for Ceph cluster (max {} attempts)", max_attempts)
        );

        for attempt in 1..=max_attempts {
            crate::log_debug!(
                "ceph_client",
                &format!(
                    "Cluster availability check: attempt {}/{}",
                    attempt, max_attempts
                )
            );

            if self.is_available().await {
                crate::log_info!("ceph_client", "Ceph cluster is available and ready");
                return Ok(());
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        crate::log_error!(
            "ceph_client",
            &format!("Ceph cluster not available after {} attempts", max_attempts)
        );
        Err(anyhow!(
            "Ceph cluster not available after {} attempts",
            max_attempts
        ))
    }
}
