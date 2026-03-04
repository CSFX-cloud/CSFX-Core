use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}

pub struct ApiClient {
    client: Client,
    gateway_url: String,
}

impl ApiClient {
    pub fn new(gateway_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self { client, gateway_url })
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

    pub async fn heartbeat(&self, agent_id: Uuid, api_key: &str) -> Result<()> {
        let url = format!(
            "{}/api/registry/agents/{}/heartbeat",
            self.gateway_url, agent_id
        );

        let resp = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .json(&HeartbeatRequest { status: None })
            .send()
            .await
            .context("Failed to send heartbeat")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!("Heartbeat failed status={}", status);
        }

        Ok(())
    }
}
