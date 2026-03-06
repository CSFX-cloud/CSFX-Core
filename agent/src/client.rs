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
    ) -> Result<()> {
        let url = format!(
            "{}/api/registry/agents/{}/heartbeat",
            self.gateway_url, agent_id
        );

        let mut req = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .json(&HeartbeatRequest {
                status: None,
                container_statuses,
            });

        if let Some(ref cert_pem) = self.cert_pem {
            req = req.header("X-Client-Cert", cert_pem.as_str());
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
        let url = format!("{}/api/workloads", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .context("Failed to fetch workloads")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!("Failed to fetch workloads status={}", status);
        }

        let all: Vec<AssignedWorkload> = resp
            .json()
            .await
            .context("Failed to parse workloads response")?;

        Ok(all
            .into_iter()
            .filter(|w| {
                w.status == "scheduled"
                    && w.container_id.is_none()
            })
            .collect())
    }

    pub async fn fetch_assigned_volumes(
        &self,
        agent_id: Uuid,
        api_key: &str,
    ) -> Result<Vec<AssignedVolume>> {
        let url = format!("{}/api/volumes", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .context("Failed to fetch volumes")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!("Failed to fetch volumes status={}", status);
        }

        let all: Vec<AssignedVolume> = resp
            .json()
            .await
            .context("Failed to parse volumes response")?;

        Ok(all
            .into_iter()
            .filter(|v| {
                v.status == "in_use"
                    && v.mapped_device.is_none()
            })
            .collect())
    }
}
