use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::runtime::VolumeUsage;

#[derive(Debug)]
pub struct GatewayConfigRejected;

impl std::fmt::Display for GatewayConfigRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("gateway config request rejected by the control plane")
    }
}

impl std::error::Error for GatewayConfigRejected {}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AssignedVolume {
    pub id: String,
    pub pool: String,
    pub image_name: String,
    pub status: String,
    #[serde(default)]
    pub size_gb: i64,
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
    machine_id: Option<&'a str>,
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
    ssh_host_key: Option<String>,
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
    pub volume_usage: Vec<VolumeUsage>,
}

#[derive(Debug, Serialize)]
struct WorkloadStatsRequest {
    stats: Vec<WorkloadStatsUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct SignedRelease {
    pub manifest: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatResponse {
    #[serde(default)]
    pub worker_releases: HashMap<String, SignedRelease>,
    #[serde(default)]
    pub ssh_enabled: bool,
    #[serde(default)]
    pub gateway_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VolumeMount {
    pub volume_id: String,
    pub mount_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AssignedWorkload {
    pub id: String,
    pub image: String,
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    #[serde(default)]
    pub cpu_limit_millicores: Option<i32>,
    #[serde(default)]
    pub memory_limit_bytes: Option<i64>,
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
    #[serde(default)]
    pub guest_ip: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceGroupPeer {
    pub agent_id: String,
    pub wg_public_key: String,
    pub wg_endpoint: String,
    pub wg_tunnel_ip: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceGroupMember {
    pub agent_id: String,
    pub service_name: Option<String>,
    pub guest_ip: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SiteService {
    pub service_name: String,
    pub address: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VpnPeer {
    pub client_public_key: String,
    pub client_tunnel_ip: String,
}

#[cfg(test)]
pub(crate) fn install_test_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub(crate) fn build_http_client(ca_pem: Option<&str>) -> Result<Client> {
    let builder = Client::builder().timeout(std::time::Duration::from_secs(30));
    let builder = match ca_pem {
        Some(pem) => builder.tls_certs_only([
            reqwest::Certificate::from_pem(pem.as_bytes()).context("Invalid cluster CA")?
        ]),
        None => builder.danger_accept_invalid_certs(true),
    };
    builder.build().context("Failed to build HTTP client")
}

#[derive(Debug, Deserialize)]
pub struct MasterBundle {
    pub ca_cert_pem: String,
    pub ca_key_pem: String,
    pub jwt_secret: String,
    pub garage_admin_token: String,
    pub object_storage_encryption_key: String,
    pub db_password: String,
    pub garage_rpc_secret: String,
    pub patroni_superuser_password: String,
    pub patroni_replication_password: String,
    pub openbao_seal_key: String,
    pub openbao_role_id: String,
    pub openbao_secret_id: String,
    #[serde(default)]
    pub site_secret: Option<String>,
    pub etcd_name: String,
    pub etcd_initial_cluster: String,
    #[serde(default)]
    pub vip: Option<ClusterVip>,
}

#[derive(Debug, Deserialize)]
pub struct ClusterVip {
    pub address: String,
    pub prefix_length: u8,
    pub router_id: u8,
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

#[derive(Debug, serde::Serialize)]
pub struct JournalEntry {
    pub level: String,
    pub unit: String,
    pub message: String,
    pub timestamp_micros: Option<i64>,
}

#[derive(serde::Serialize)]
struct JournalBatch {
    entries: Vec<JournalEntry>,
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

    pub fn gateway_url(&self) -> &str {
        &self.gateway_url
    }

    pub async fn request_master_bundle(&self, api_key: &str) -> Result<MasterBundle> {
        let url = format!("{}/api/cluster/masters/join", self.gateway_url);

        let resp = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to request master join bundle")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Master join bundle request failed status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json()
            .await
            .context("Failed to parse master join bundle")
    }

    pub async fn fetch_cluster_vip(&self, api_key: &str) -> Result<Option<ClusterVip>> {
        let url = format!("{}/api/cluster/masters/vip", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to request cluster vip")?;

        if !resp.status().is_success() {
            anyhow::bail!("Cluster vip request failed status={}", resp.status());
        }

        resp.json().await.context("Failed to parse cluster vip")
    }

    pub async fn promote_master(&self, api_key: &str) -> Result<bool> {
        let url = format!("{}/api/cluster/masters/promote", self.gateway_url);

        let resp = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to request master promotion")?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            return Ok(false);
        }
        if !resp.status().is_success() {
            anyhow::bail!("Master promotion failed status={}", resp.status());
        }
        Ok(true)
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
        machine_id: Option<&str>,
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
            machine_id,
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
                ssh_host_key: crate::system::read_ssh_host_key(),
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

    pub async fn fetch_resource_group_members(
        &self,
        api_key: &str,
        resource_group_id: &str,
    ) -> Result<Vec<ResourceGroupMember>> {
        let url = format!(
            "{}/api/resource-groups/{}/members",
            self.gateway_url, resource_group_id
        );

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch resource group members")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch resource group members status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<Vec<ResourceGroupMember>>()
            .await
            .context("Failed to parse resource group members response")
    }

    pub async fn fetch_resource_group_site_services(
        &self,
        api_key: &str,
        resource_group_id: &str,
    ) -> Result<Vec<SiteService>> {
        let url = format!(
            "{}/api/resource-groups/{}/site-services",
            self.gateway_url, resource_group_id
        );

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch resource group site services")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch resource group site services status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<Vec<SiteService>>()
            .await
            .context("Failed to parse resource group site services response")
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

    pub async fn fetch_gateway_config(
        &self,
        api_key: &str,
        resource_group_id: &str,
        gateway_id: &str,
        source_workload_id: &str,
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}/api/resource-groups/{}/gateways/{}/config",
            self.gateway_url, resource_group_id, gateway_id
        );

        let resp = self
            .client
            .get(&url)
            .query(&[("source_workload_id", source_workload_id)])
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch ingress gateway config")?;

        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(GatewayConfigRejected.into());
        }
        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch ingress gateway config status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<serde_json::Value>()
            .await
            .context("Failed to parse ingress gateway config response")
    }

    pub async fn fetch_mesh_peers(&self, api_key: &str) -> Result<Vec<ResourceGroupPeer>> {
        let url = format!("{}/api/agents/mesh-peers", self.gateway_url);

        let resp = self
            .client
            .get(&url)
            .header("X-API-Key", api_key)
            .send()
            .await
            .context("Failed to fetch mesh peers")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!(
                "Failed to fetch mesh peers status={} {}",
                status,
                resp.text().await.unwrap_or_default()
            );
        }

        resp.json::<Vec<ResourceGroupPeer>>()
            .await
            .context("Failed to parse mesh peers response")
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

    pub async fn push_logs(&self, api_key: &str, entries: Vec<JournalEntry>) -> Result<()> {
        let url = format!("{}/api/agents/self/logs", self.gateway_url);

        let resp = self
            .client
            .post(&url)
            .header("X-API-Key", api_key)
            .json(&JournalBatch { entries })
            .send()
            .await
            .context("Failed to push logs")?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to push logs status={}", resp.status());
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

        Ok(all.into_iter().filter(|v| v.status == "in_use").collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    async fn serve(app: Router) -> ApiClient {
        install_test_crypto_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        ApiClient::new(
            url,
            "pub".to_string(),
            None,
            Some("10.9.0.1".to_string()),
            None,
        )
        .unwrap()
    }

    async fn failing_client() -> ApiClient {
        serve(Router::new().fallback(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "nope") }))
            .await
    }

    fn bundle() -> serde_json::Value {
        json!({
            "ca_cert_pem": "ca", "ca_key_pem": "key", "jwt_secret": "jwt",
            "garage_admin_token": "g", "object_storage_encryption_key": "o",
            "db_password": "db", "garage_rpc_secret": "rpc",
            "patroni_superuser_password": "ps", "patroni_replication_password": "pr",
            "openbao_seal_key": "s", "openbao_role_id": "r", "openbao_secret_id": "i",
            "etcd_name": "etcd1", "etcd_initial_cluster": "etcd1=http://x",
            "vip": {"address": "10.0.0.9", "prefix_length": 24, "router_id": 7}
        })
    }

    fn workload() -> serde_json::Value {
        json!({
            "id": "w1", "image": "nginx", "cpu_millicores": 500, "memory_bytes": 1024,
            "restart_policy": "always", "disk_bytes": 4096,
            "ports": [{"container_port": 80, "protocol": "tcp"}],
            "env_vars": {"A": "b"}
        })
    }

    fn gateway() -> Router {
        Router::new()
            .route("/api/setup/ca.crt", get(|| async { "PEM" }))
            .route(
                "/api/cluster/masters/join",
                post(|| async { Json(bundle()) }),
            )
            .route(
                "/api/cluster/masters/vip",
                get(|| async {
                    Json(json!({"address": "10.0.0.9", "prefix_length": 24, "router_id": 1}))
                }),
            )
            .route(
                "/api/cluster/masters/promote",
                post(|| async { StatusCode::OK }),
            )
            .route(
                "/api/resource-groups/{id}/members",
                get(|Path(id): Path<String>| async move {
                    Json(json!([{"agent_id": id, "service_name": "web", "guest_ip": "10.0.0.3"}]))
                }),
            )
            .route(
                "/api/resource-groups/{id}/site-services",
                get(|| async { Json(json!([{"service_name": "s", "address": "10.0.0.4"}])) }),
            )
            .route(
                "/api/resource-groups/{id}/peers",
                get(|| async { Json(peers()) }),
            )
            .route("/api/agents/mesh-peers", get(|| async { Json(peers()) }))
            .route(
                "/api/resource-groups/{id}/gateways/{gateway}/config",
                get(|| async { Json(json!({"routes": []})) }),
            )
            .route(
                "/api/resource-groups/{id}/vpn-peers",
                get(|| async {
                    Json(json!([{"client_public_key": "k", "client_tunnel_ip": "10.8.0.2"}]))
                }),
            )
            .route(
                "/api/resource-groups/agent/active-ids",
                get(|| async { Json(json!(["rg1", "rg2"])) }),
            )
            .route(
                "/api/agents/self/workloads",
                get(|| async { Json(json!([workload()])) }),
            )
            .route(
                "/api/agents/self/workloads/stats",
                post(|| async { StatusCode::OK }),
            )
            .route(
                "/api/agents/self/workloads/{id}/restart-ack",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route("/api/agents/self/logs", post(|| async { StatusCode::OK }))
            .route(
                "/api/agents/self/volumes",
                get(|| async { Json(volumes()) }),
            )
    }

    fn peers() -> serde_json::Value {
        json!([{
            "agent_id": "a", "wg_public_key": "k",
            "wg_endpoint": "1.2.3.4:5", "wg_tunnel_ip": "10.9.0.2"
        }])
    }

    fn volumes() -> serde_json::Value {
        json!([
            {"id": "1", "pool": "p", "image_name": "i", "status": "in_use"},
            {"id": "2", "pool": "p", "image_name": "i", "status": "in_use"},
            {"id": "3", "pool": "p", "image_name": "i", "status": "available"}
        ])
    }

    #[tokio::test]
    async fn cluster_ca_and_master_endpoints() {
        let client = serve(gateway()).await;
        assert_eq!(fetch_cluster_ca(client.gateway_url()).await.unwrap(), "PEM");
        let bundle = client.request_master_bundle("key").await.unwrap();
        assert_eq!(bundle.etcd_name, "etcd1");
        assert_eq!(bundle.vip.unwrap().router_id, 7);
        let vip = client.fetch_cluster_vip("key").await.unwrap().unwrap();
        assert_eq!(vip.address, "10.0.0.9");
        assert!(client.promote_master("key").await.unwrap());
    }

    #[tokio::test]
    async fn promotion_conflict_is_not_an_error() {
        let app = Router::new().route(
            "/api/cluster/masters/promote",
            post(|| async { StatusCode::CONFLICT }),
        );
        assert!(!serve(app).await.promote_master("key").await.unwrap());
    }

    #[tokio::test]
    async fn resource_group_endpoints_parse_responses() {
        let client = serve(gateway()).await;
        let members = client
            .fetch_resource_group_members("k", "rg")
            .await
            .unwrap();
        assert_eq!(members[0].agent_id, "rg");
        let services = client
            .fetch_resource_group_site_services("k", "rg")
            .await
            .unwrap();
        assert_eq!(services[0].address, "10.0.0.4");
        let peers = client.fetch_resource_group_peers("k", "rg").await.unwrap();
        assert_eq!(peers[0].wg_endpoint, "1.2.3.4:5");
        assert_eq!(client.fetch_mesh_peers("k").await.unwrap().len(), 1);
        let config = client
            .fetch_gateway_config("k", "rg", "gw", "wl")
            .await
            .unwrap();
        assert!(config["routes"].is_array());
        let vpn = client
            .fetch_resource_group_vpn_peers("k", "rg")
            .await
            .unwrap();
        assert_eq!(vpn[0].client_tunnel_ip, "10.8.0.2");
        let ids = client.fetch_active_resource_group_ids("k").await.unwrap();
        assert_eq!(ids, vec!["rg1".to_string(), "rg2".to_string()]);
    }

    #[tokio::test]
    async fn workload_and_volume_endpoints() {
        let client = serve(gateway()).await;
        let workloads = client.fetch_assigned_workloads("k").await.unwrap();
        assert_eq!(workloads[0].image, "nginx");
        assert!(!workloads[0].restart_requested);
        assert_eq!(workloads[0].env_vars.as_ref().unwrap()["A"], "b");
        let volumes = client
            .fetch_assigned_volumes(Uuid::new_v4(), "k")
            .await
            .unwrap();
        assert_eq!(volumes.len(), 2);
        assert_eq!(volumes[0].id, "1");
        assert_eq!(volumes[1].id, "2");
        client.ack_workload_restart("k", "w1").await.unwrap();
    }

    #[tokio::test]
    async fn push_endpoints_skip_empty_payloads() {
        let client = serve(gateway()).await;
        client.push_workload_stats("k", Vec::new()).await.unwrap();
        let stat = WorkloadStatsUpdate {
            workload_id: "w".to_string(),
            cpu_usage_percent: Some(1.0),
            memory_usage_bytes: None,
            network_rx_bytes: None,
            network_tx_bytes: None,
            volume_usage: Vec::new(),
        };
        client.push_workload_stats("k", vec![stat]).await.unwrap();
        let entry = JournalEntry {
            level: "ERROR".to_string(),
            unit: "u".to_string(),
            message: "m".to_string(),
            timestamp_micros: Some(1),
        };
        client.push_logs("k", vec![entry]).await.unwrap();
    }

    #[tokio::test]
    async fn every_endpoint_reports_server_errors() {
        let client = failing_client().await;
        assert!(fetch_cluster_ca(client.gateway_url()).await.is_err());
        assert!(client.request_master_bundle("k").await.is_err());
        assert!(client.fetch_cluster_vip("k").await.is_err());
        assert!(client.promote_master("k").await.is_err());
        assert!(client.fetch_resource_group_members("k", "r").await.is_err());
        assert!(client
            .fetch_resource_group_site_services("k", "r")
            .await
            .is_err());
        assert!(client.fetch_resource_group_peers("k", "r").await.is_err());
        assert!(client
            .fetch_gateway_config("k", "r", "g", "w")
            .await
            .is_err());
        assert!(client.fetch_mesh_peers("k").await.is_err());
        assert!(client
            .fetch_resource_group_vpn_peers("k", "r")
            .await
            .is_err());
        assert!(client.fetch_active_resource_group_ids("k").await.is_err());
        assert!(client.fetch_assigned_workloads("k").await.is_err());
        assert!(client
            .fetch_assigned_volumes(Uuid::nil(), "k")
            .await
            .is_err());
        assert!(client.ack_workload_restart("k", "w").await.is_err());
        assert!(client.push_logs("k", Vec::new()).await.is_err());
        let stat = WorkloadStatsUpdate {
            workload_id: "w".to_string(),
            cpu_usage_percent: None,
            memory_usage_bytes: None,
            network_rx_bytes: None,
            network_tx_bytes: None,
            volume_usage: Vec::new(),
        };
        assert!(client.push_workload_stats("k", vec![stat]).await.is_err());
    }

    #[tokio::test]
    async fn malformed_bodies_are_parse_errors() {
        let app = Router::new().fallback(|| async { "not json" });
        let client = serve(app).await;
        assert!(client.request_master_bundle("k").await.is_err());
        assert!(client.fetch_mesh_peers("k").await.is_err());
    }

    #[tokio::test]
    async fn registration_retries_after_rate_limiting() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let app = Router::new().route(
            "/api/registry/agents/register",
            post(move || {
                let counter = counter.clone();
                async move {
                    if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                        return (
                            StatusCode::TOO_MANY_REQUESTS,
                            [("Retry-After", "0")],
                            String::new(),
                        )
                            .into_response();
                    }
                    Json(json!({
                        "agent_id": Uuid::nil(), "api_key": "secret",
                        "certificate_pem": "cert", "ca_cert_pem": "ca", "wg_tunnel_ip": "10.9.0.5"
                    }))
                    .into_response()
                }
            }),
        );
        let client = serve(app).await;
        let response = client
            .register("t", "n", "h", "linux", "1", "x86_64", Some("mid"), "csr")
            .await
            .unwrap();
        assert_eq!(response.api_key, "secret");
        assert_eq!(response.wg_tunnel_ip.as_deref(), Some("10.9.0.5"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn registration_failures_include_the_body() {
        let client =
            serve(Router::new().fallback(|| async { (StatusCode::FORBIDDEN, "denied") })).await;
        let error = client
            .register("t", "n", "h", "linux", "1", "x86_64", None, "csr")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("403"));
        assert!(error.contains("denied"));
    }

    #[tokio::test]
    async fn heartbeat_carries_identity_metrics_and_certificate() {
        let seen = Arc::new(std::sync::Mutex::new(None));
        let sink = seen.clone();
        let app = Router::new().route(
            "/api/registry/agents/{id}/heartbeat",
            post(
                move |headers: HeaderMap, Json(body): Json<serde_json::Value>| {
                    let sink = sink.clone();
                    async move {
                        *sink.lock().unwrap() = Some((headers, body));
                        Json(json!({
                            "worker_releases": {"x86_64-linux": {"manifest": "m", "signature": "s"}},
                            "ssh_enabled": true, "gateway_url": "https://gw"
                        }))
                    }
                },
            ),
        );
        let client = serve(app)
            .await
            .with_certificate("line1\nline2".to_string());
        client.set_wg_tunnel_ip("10.9.0.7".to_string()).await;
        let statuses = vec![ContainerStatus {
            workload_id: "w".to_string(),
            container_id: "c".to_string(),
            status: "running".to_string(),
        }];
        let response = client
            .heartbeat(
                Uuid::new_v4(),
                "key",
                Some(statuses),
                Some(crate::system::collect_metrics()),
            )
            .await
            .unwrap();
        assert!(response.ssh_enabled);
        assert_eq!(response.worker_releases["x86_64-linux"].manifest, "m");
        assert_eq!(response.gateway_url.as_deref(), Some("https://gw"));

        let (headers, body) = seen.lock().unwrap().take().unwrap();
        assert_eq!(headers["x-api-key"], "key");
        assert_eq!(headers["x-client-cert"], "line1\\nline2");
        assert_eq!(body["wg_public_key"], "pub");
        assert_eq!(body["wg_tunnel_ip"], "10.9.0.7");
        assert_eq!(body["container_statuses"][0]["workload_id"], "w");
        assert!(body["cpu_cores"].is_number());
    }

    #[tokio::test]
    async fn heartbeat_without_metrics_and_failure() {
        let app = Router::new().route(
            "/api/registry/agents/{id}/heartbeat",
            post(|| async { Json(json!({})) }),
        );
        let ok = serve(app)
            .await
            .heartbeat(Uuid::nil(), "k", None, None)
            .await
            .unwrap();
        assert!(ok.worker_releases.is_empty());
        assert!(!ok.ssh_enabled);
        let failed = failing_client()
            .await
            .heartbeat(Uuid::nil(), "k", None, None)
            .await;
        assert!(failed.is_err());
    }

    #[test]
    fn http_client_validates_the_cluster_ca() {
        install_test_crypto_provider();
        assert!(build_http_client(None).is_ok());
        let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let cert = rcgen::CertificateParams::default()
            .self_signed(&key)
            .unwrap();
        assert!(build_http_client(Some(&cert.pem())).is_ok());
    }
}
