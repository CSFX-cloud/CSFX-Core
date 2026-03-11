use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContainerStatus {
    pub workload_id: String,
    pub container_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignedWorkload {
    pub id: String,
    pub name: String,
    pub image: String,
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    pub disk_bytes: i64,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<crate::docker::PortMapping>>,
    pub status: String,
    pub container_id: Option<String>,
}

pub struct ApiClient {
    client: Client,
    gateway_url: String,
    cert_pem: Option<String>,
}

impl ApiClient {
    pub fn new(gateway_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            gateway_url,
            cert_pem: None,
        })
    }

    pub fn with_certificate(mut self, cert_pem: String) -> Self {
        self.cert_pem = Some(cert_pem);
        self
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

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send registration request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Registration failed status={} body={}", status, body);
        }

        resp.json::<RegisterResponse>()
            .await
            .context("Failed to parse registration response")
    }

    pub async fn heartbeat(
        &self,
        agent_id: Uuid,
        api_key: &str,
        container_statuses: Option<Vec<ContainerStatus>>,
        metrics: Option<crate::system::SystemMetrics>,
    ) -> Result<()> {
        let url = format!(
            "{}/api/registry/agents/{}/heartbeat",
            self.gateway_url, agent_id
        );

        let (cpu_usage_percent, cpu_cores, memory_total_bytes, memory_used_bytes,
             disk_total_bytes, disk_used_bytes, network_rx_bytes, network_tx_bytes,
             uptime_seconds) = metrics.map(|m| (
            Some(m.cpu_usage_percent), Some(m.cpu_cores),
            Some(m.memory_total_bytes), Some(m.memory_used_bytes),
            Some(m.disk_total_bytes), Some(m.disk_used_bytes),
            Some(m.network_rx_bytes), Some(m.network_tx_bytes),
            Some(m.uptime_seconds),
        )).unwrap_or_default();

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

        Ok(())
    }

    pub async fn fetch_assigned_workloads(
        &self,
        api_key: &str,
    ) -> Result<Vec<AssignedWorkload>> {
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
            anyhow::bail!("Failed to fetch workloads status={} {}", status, resp.text().await.unwrap_or_default());
        }

        let all: Vec<AssignedWorkload> = resp
            .json()
            .await
            .context("Failed to parse workloads response")?;

        Ok(all
            .into_iter()
            .filter(|w| w.status == "scheduled" && w.container_id.is_none())
            .collect())
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
            anyhow::bail!("Failed to fetch volumes status={} {}", status, resp.text().await.unwrap_or_default());
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
