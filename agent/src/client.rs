use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct AssignedVolume {
    pub id: String,
    pub name: String,
    pub pool: String,
    pub image_name: String,
    pub status: String,
    pub mapped_device: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegisterRequest<'a> {
    registration_token: &'a str,
    name: &'a str,
    hostname: &'a str,
    os_type: &'a str,
    os_version: &'a str,
    architecture: &'a str,
    agent_version: &'a str,
    csr_pem: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct RegisterResponse {
    pub agent_id: Uuid,
    pub api_key: String,
    pub certificate_pem: Option<String>,
    pub ca_cert_pem: Option<String>,
    pub wg_tunnel_ip: Option<String>,
}

#[derive(Debug, Serialize)]
struct HeartbeatRequest {
    status: Option<String>,
    container_statuses: Option<Vec<ContainerStatus>>,
    cpu_usage_percent: Option<f32>,
    cpu_cores: Option<u32>,
    memory_total_bytes: Option<u64>,
    memory_used_bytes: Option<u64>,
    disk_total_bytes: Option<u64>,
    disk_used_bytes: Option<u64>,
    network_rx_bytes: Option<u64>,
    network_tx_bytes: Option<u64>,
    uptime_seconds: Option<u64>,
    wg_public_key: Option<String>,
    wg_endpoint: Option<String>,
    wg_tunnel_ip: Option<String>,
    agent_version: Option<String>,
    kvm_capable: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContainerStatus {
    pub workload_id: String,
    pub container_id: String,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkloadStatsUpdate {
    pub workload_id: String,
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_bytes: Option<i64>,
    pub network_rx_bytes: Option<i64>,
    pub network_tx_bytes: Option<i64>,
}

#[derive(Debug, Serialize)]
struct WorkloadStatsRequest {
    stats: Vec<WorkloadStatsUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatResponse {
    pub desired_flake_rev: Option<String>,
    pub post_update_heartbeats: Option<u32>,
    #[serde(default)]
    pub ssh_enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VolumeMount {
    pub volume_id: String,
    pub mount_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssignedWorkload {
    pub id: String,
    pub image: String,
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<crate::spec::PortMapping>>,
    pub volume_mounts: Option<Vec<VolumeMount>>,
    pub service_name: Option<String>,
    pub restart_policy: String,
    pub max_restarts: Option<i32>,
    pub resource_group_id: Option<String>,
    pub resource_group_cidr: Option<String>,
    #[serde(default)]
    pub restart_requested: bool,
    #[serde(default)]
    pub runtime_class: String,
    pub disk_bytes: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceGroupPeer {
    pub agent_id: String,
    pub wg_public_key: String,
    pub wg_endpoint: String,
    pub wg_tunnel_ip: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VpnPeer {
    pub client_public_key: String,
    pub client_tunnel_ip: String,
}

fn build_http_client(ca_pem: Option<&str>) -> Result<Client> {
    let builder = Client::builder().timeout(std::time::Duration::from_secs(30));
    let builder = match ca_pem {
        Some(pem) => builder.tls_certs_only([
            reqwest::Certificate::from_pem(pem.as_bytes()).context("Invalid cluster CA")?,
        ]),
        None => builder.danger_accept_invalid_certs(true),
    };
    builder.build().context("Failed to build HTTP client")
}

pub async fn fetch_cluster_ca(gateway_url: &str) -> Result<String> {
    let url = format!("{}/api/setup/ca.crt", gateway_url);

    let resp = build_http_client(None)?
        .get(&url)
        .send()
        .await
        .context("Failed to fetch cluster CA")?;

    if !resp.status().is_success() {
        anyhow::bail!("Cluster CA fetch failed status={}", resp.status());
    }

    resp.text().await.context("Failed to read cluster CA")
}

pub struct ApiClient {
    client: Client,
    gateway_url: String,
    cert_pem: Option<String>,
    wg_public_key: String,
    wg_endpoint: Option<String>,
    wg_tunnel_ip: RwLock<Option<String>>,
}

impl ApiClient {
    pub fn new(
        gateway_url: String,
        wg_public_key: String,
        wg_endpoint: Option<String>,
        wg_tunnel_ip: Option<String>,
        ca_pem: Option<&str>,
    ) -> Result<Self> {
        let client = build_http_client(ca_pem)?;

        Ok(Self {
            client,
            gateway_url,
            cert_pem: None,
            wg_public_key,
            wg_endpoint,
            wg_tunnel_ip: RwLock::new(wg_tunnel_ip),
        })
    }

    pub fn with_certificate(mut self, cert_pem: String) -> Self {
        self.cert_pem = Some(cert_pem);
        self
    }

    pub async fn set_wg_tunnel_ip(&self, ip: String) {
        *self.wg_tunnel_ip.write().await = Some(ip);
    }

    pub async fn register(
        &self,
        token: &str,
        name: &str,
        hostname: &str,
        os_type: &str,
        os_version: &str,
        architecture: &str,
        csr_pem: &str,
    ) -> Result<RegisterResponse> {
        let url = format!("{}/api/registry/agents/register", self.gateway_url);

        let body = RegisterRequest {
            registration_token: token,
            name,
            hostname,
            os_type,
            os_version,
            architecture,
            agent_version: env!("CARGO_PKG_VERSION"),
            csr_pem,
        };

        loop {
            let resp = self
                .client
                .post(&url)
                .json(&body)
                .send()
                .await
                .context("Failed to send registration request")?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60);
                tracing::warn!(retry_after, "Registration rate-limited, retrying");
                tokio::time::sleep(Duration::from_secs(retry_after)).await;
                continue;
            }

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("Registration failed status={} body={}", status, body);
            }

            return resp
                .json::<RegisterResponse>()
                .await
                .context("Failed to parse registration response");
        }
    }

    pub async fn heartbeat(
        &self,
        agent_id: Uuid,
        api_key: &str,
        container_statuses: Option<Vec<ContainerStatus>>,
        metrics: Option<crate::system::SystemMetrics>,
    ) -> Result<HeartbeatResponse> {
        let url = format!(
            "{}/api/registry/agents/{}/heartbeat",
            self.gateway_url, agent_id
        );

        let (
            cpu_usage_percent,
            cpu_cores,
            memory_total_bytes,
            memory_used_bytes,
            disk_total_bytes,
            disk_used_bytes,
            network_rx_bytes,
            network_tx_bytes,
            uptime_seconds,
        ) = metrics
            .map(|m| {
                (
                    Some(m.cpu_usage_percent),
                    Some(m.cpu_cores),
                    Some(m.memory_total_bytes),
                    Some(m.memory_used_bytes),
                    Some(m.disk_total_bytes),
                    Some(m.disk_used_bytes),
                    Some(m.network_rx_bytes),
                    Some(m.network_tx_bytes),
                    Some(m.uptime_seconds),
                )
            })
            .unwrap_or_default();

        let wg_tunnel_ip = self.wg_tunnel_ip.read().await.clone();

        let mut req = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .json(&HeartbeatRequest {
                status: None,
                container_statuses,
                cpu_usage_percent,
                cpu_cores,
                memory_total_bytes,
                memory_used_bytes,
                disk_total_bytes,
                disk_used_bytes,
                network_rx_bytes,
                network_tx_bytes,
                uptime_seconds,
                wg_public_key: Some(self.wg_public_key.clone()),
                wg_endpoint: self.wg_endpoint.clone(),
                wg_tunnel_ip,
                agent_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                kvm_capable: crate::system::is_kvm_capable(),
            });

        if let Some(ref cert_pem) = self.cert_pem {
            let encoded = cert_pem.replace('\n', "\\n");
            req = req.header("X-Client-Cert", encoded);
        }

        let resp = req.send().await.context("Failed to send heartbeat")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!("Heartbeat failed status={}", status);
        }

        resp.json::<HeartbeatResponse>()
            .await
            .context("Failed to parse heartbeat response")
    }

    pub async fn fetch_resource_group_peers(
        &self,
        api_key: &str,
        resource_group_id: &str,
    ) -> Result<Vec<ResourceGroupPeer>> {
        let url = format!(
            "{}/api/resource-groups/{}/peers",
            self.gateway_url, resource_group_id
        );

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch resource group peers")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch resource group peers status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<Vec<ResourceGroupPeer>>()
            .await
            .context("Failed to parse resource group peers response")
    }

    pub async fn fetch_resource_group_vpn_peers(
        &self,
        api_key: &str,
        resource_group_id: &str,
    ) -> Result<Vec<VpnPeer>> {
        let url = format!(
            "{}/api/resource-groups/{}/vpn-peers",
            self.gateway_url, resource_group_id
        );

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch resource group vpn peers")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch resource group vpn peers status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<Vec<VpnPeer>>()
            .await
            .context("Failed to parse resource group vpn peers response")
    }

    pub async fn fetch_active_resource_group_ids(&self, api_key: &str) -> Result<Vec<String>> {
        let url = format!("{}/api/resource-groups/agent/active-ids", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch active resource group ids")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch active resource group ids status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<Vec<String>>()
            .await
            .context("Failed to parse active resource group ids response")
    }

    pub async fn fetch_assigned_workloads(&self, api_key: &str) -> Result<Vec<AssignedWorkload>> {
        let url = format!("{}/api/agents/self/workloads", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch workloads")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch workloads status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<Vec<AssignedWorkload>>()
            .await
            .context("Failed to parse workloads response")
    }

    pub async fn push_workload_stats(
        &self,
        api_key: &str,
        stats: Vec<WorkloadStatsUpdate>,
    ) -> Result<()> {
        if stats.is_empty() {
            return Ok(());
        }

        let url = format!("{}/api/agents/self/workloads/stats", self.gateway_url);

        let resp = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .json(&WorkloadStatsRequest { stats })
            .send()
            .await
            .context("Failed to push workload stats")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to push workload stats status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        Ok(())
    }

    pub async fn ack_workload_restart(&self, api_key: &str, workload_id: &str) -> Result<()> {
        let url = format!(
            "{}/api/agents/self/workloads/{}/restart-ack",
            self.gateway_url, workload_id
        );

        let resp = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to ack workload restart")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to ack workload restart status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        Ok(())
    }

    pub async fn fetch_bootstrap_token(&self) -> Result<String> {
        let url = format!("{}/api/registry/internal/bootstrap-token", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch bootstrap token")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!("Bootstrap token fetch failed status={}", status);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse bootstrap token response")?;

        body["token"]
            .as_str()
            .map(|s| s.to_string())
            .context("Bootstrap token response missing 'token' field")
    }

    pub async fn fetch_assigned_volumes(
        &self,
        _agent_id: Uuid,
        api_key: &str,
    ) -> Result<Vec<AssignedVolume>> {
        let url = format!("{}/api/agents/self/volumes", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch volumes")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch volumes status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        let all: Vec<AssignedVolume> = resp
            .json()
            .await
            .context("Failed to parse volumes response")?;

        Ok(all
            .into_iter()
            .filter(|v| v.status == "in_use" && v.mapped_device.is_none())
            .collect())
    }
}
