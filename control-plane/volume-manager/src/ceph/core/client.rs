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

    /// Führt ein Ceph-Kommando aus
    pub async fn execute(&self, cmd: CephCommand) -> Result<String> {
        let mut command = AsyncCommand::new("ceph");

        // Monitoring hosts hinzufügen
        command.arg("-m").arg(self.config.mon_host_string());

        // Keyring falls vorhanden
        if let Some(ref keyring) = self.config.keyring_path {
            command.arg("--keyring").arg(keyring);
        }

        // Client name
        command
            .arg("--name")
            .arg(format!("client.{}", self.config.client_name));

        // Das eigentliche Kommando
        for arg in cmd.to_vec() {
            command.arg(arg);
        }

        // JSON Format für strukturierte Ausgabe
        command.arg("--format").arg("json");

        let output = command
            .output()
            .await
            .context("Failed to execute ceph command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Ceph command failed: {}", stderr));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    /// Prüft Cluster-Health
    pub async fn health_status(&self) -> Result<CephClusterHealth> {
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

    /// Prüft ob Ceph-Cluster erreichbar ist
    pub async fn is_available(&self) -> bool {
        self.health_status().await.is_ok()
    }

    /// Wartet bis Cluster verfügbar ist
    pub async fn wait_for_cluster(&self, max_attempts: u32) -> Result<()> {
        for attempt in 1..=max_attempts {
            crate::log_info!(
                "ceph_client",
                &format!(
                    "Waiting for Ceph cluster... (attempt {}/{})",
                    attempt, max_attempts
                )
            );

            if self.is_available().await {
                crate::log_info!("ceph_client", "Ceph cluster is available");
                return Ok(());
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        Err(anyhow!(
            "Ceph cluster not available after {} attempts",
            max_attempts
        ))
    }
}
