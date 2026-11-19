use anyhow::{Context, Result};
use reqwest::header::{ETAG, IF_NONE_MATCH};
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

pub use csfx_agent_api::{
    ContainerStatus, HeartbeatRequest, HeartbeatResponse, RegisterRequest, RegisterResponse,
    SignedRelease,
};

use crate::runtime::VolumeUsage;
use crate::system::SystemMetrics;

fn is_throttled(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
}

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
    #[serde(default, deserialize_with = "readable_ports")]
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
    #[serde(default)]
    pub replaces_workload_id: Option<String>,
    #[serde(default)]
    pub env_unresolved: bool,
}

fn readable_ports<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<crate::spec::PortMapping>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(items) = Option::<Vec<serde_json::Value>>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let ports = items
        .into_iter()
        .filter_map(|item| match serde_json::from_value(item) {
            Ok(port) => Some(port),
            Err(error) => {
                warn!(error = %error, "skipping unreadable port mapping");
                None
            }
        })
        .collect();
    Ok(Some(ports))
}

fn parse_workloads(items: Vec<serde_json::Value>) -> Result<Vec<AssignedWorkload>> {
    let mut workloads = Vec::with_capacity(items.len());
    let mut unreadable = Vec::new();
    for item in items {
        let workload_id = item
            .get("id")
            .and_then(|id| id.as_str())
            .map(str::to_string);
        match serde_json::from_value(item) {
            Ok(workload) => workloads.push(workload),
            Err(error) => {
                let workload_id = workload_id.unwrap_or_else(|| "unknown".to_string());
                warn!(workload_id = %workload_id, error = %error, "unreadable workload in assignment");
                unreadable.push(workload_id);
            }
        }
    }
    if !unreadable.is_empty() {
        anyhow::bail!(
            "unreadable workloads in assignment ids={}",
            unreadable.join(",")
        );
    }
    Ok(workloads)
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceGroupPeer {
    pub agent_id: String,
    pub wg_public_key: String,
    pub wg_endpoint: String,
    pub wg_tunnel_ip: String,
    #[serde(default)]
    pub wg_listen_port: Option<u16>,
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

pub const PROTOCOL_VERSION: u32 = 1;
const PROTOCOL_HEADER: &str = "X-Csfx-Agent-Protocol";

pub(crate) fn build_http_client(ca_pem: Option<&str>) -> Result<Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(PROTOCOL_HEADER, PROTOCOL_VERSION.into());
    let builder = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .default_headers(headers);
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
    let response = build_http_client(None)?
        .get(format!("{gateway_url}/api/setup/ca.crt"))
        .send()
        .await
        .context("Failed to fetch cluster CA")?;

    check_status(response, "fetch cluster CA")
        .await?
        .text()
        .await
        .context("Failed to read cluster CA")
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
    api_key: Option<String>,
    cert_pem: Option<String>,
    wg_public_key: String,
    wg_endpoint: Option<String>,
    mesh_peers: Mutex<Option<(String, Vec<ResourceGroupPeer>)>>,
}

impl ApiClient {
    pub fn new(
        gateway_url: String,
        wg_public_key: String,
        wg_endpoint: Option<String>,
        ca_pem: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {
            client: build_http_client(ca_pem)?,
            gateway_url,
            api_key: None,
            cert_pem: None,
            wg_public_key,
            wg_endpoint,
            mesh_peers: Mutex::new(None),
        })
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_certificate(mut self, cert_pem: String) -> Self {
        self.cert_pem = Some(cert_pem);
        self
    }

    pub fn gateway_url(&self) -> &str {
        &self.gateway_url
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let builder = self
            .client
            .request(method, format!("{}{}", self.gateway_url, path));
        match &self.api_key {
            Some(api_key) => builder.header("X-API-Key", api_key),
            None => builder,
        }
    }

    async fn send_checked(&self, builder: RequestBuilder, action: &str) -> Result<Response> {
        let response = builder
            .send()
            .await
            .with_context(|| format!("Failed to {action}"))?;
        check_status(response, action).await
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        builder: RequestBuilder,
        action: &str,
    ) -> Result<T> {
        self.send_checked(builder, action)
            .await?
            .json()
            .await
            .with_context(|| format!("Failed to parse {action} response"))
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str, action: &str) -> Result<T> {
        self.send_json(self.request(Method::GET, path), action)
            .await
    }

    pub async fn request_master_bundle(&self) -> Result<MasterBundle> {
        let builder = self.request(Method::POST, "/api/cluster/masters/join");
        self.send_json(builder, "request master join bundle").await
    }

    pub async fn fetch_cluster_vip(&self) -> Result<Option<ClusterVip>> {
        self.get_json("/api/cluster/masters/vip", "fetch cluster vip")
            .await
    }

    pub async fn promote_master(&self) -> Result<bool> {
        let response = self
            .request(Method::POST, "/api/cluster/masters/promote")
            .send()
            .await
            .context("Failed to request master promotion")?;
        if response.status() == reqwest::StatusCode::CONFLICT {
            return Ok(false);
        }
        check_status(response, "promote master").await?;
        Ok(true)
    }

    pub async fn register(&self, body: &RegisterRequest) -> Result<RegisterResponse> {
        loop {
            let response = self
                .request(Method::POST, "/api/registry/agents/register")
                .json(body)
                .send()
                .await
                .context("Failed to send registration request")?;

            if !is_throttled(response.status()) {
                return check_status(response, "register")
                    .await?
                    .json()
                    .await
                    .context("Failed to parse registration response");
            }
            wait_out_throttle(&response).await;
        }
    }

    pub async fn heartbeat(
        &self,
        agent_id: Uuid,
        container_statuses: Option<Vec<ContainerStatus>>,
        metrics: Option<SystemMetrics>,
    ) -> Result<HeartbeatResponse> {
        let body = HeartbeatRequest {
            container_statuses,
            metrics,
            wg_public_key: Some(self.wg_public_key.clone()),
            wg_endpoint: self.wg_endpoint.clone(),
            agent_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            release_version: crate::update_watch::applied_version().await,
            kvm_capable: crate::system::is_kvm_capable(),
            ssh_host_key: crate::system::read_ssh_host_key(),
        };
        let path = format!("/api/registry/agents/{agent_id}/heartbeat");
        let mut builder = self.request(Method::POST, &path).json(&body);
        if let Some(cert_pem) = &self.cert_pem {
            builder = builder.header("X-Client-Cert", cert_pem.replace('\n', "\\n"));
        }
        self.send_json(builder, "send heartbeat").await
    }

    pub async fn fetch_resource_group_members(
        &self,
        resource_group_id: &str,
    ) -> Result<Vec<ResourceGroupMember>> {
        let path = format!("/api/resource-groups/{resource_group_id}/members");
        self.get_json(&path, "fetch resource group members").await
    }

    pub async fn fetch_resource_group_site_services(
        &self,
        resource_group_id: &str,
    ) -> Result<Vec<SiteService>> {
        let path = format!("/api/resource-groups/{resource_group_id}/site-services");
        self.get_json(&path, "fetch resource group site services")
            .await
    }

    pub async fn fetch_resource_group_peers(
        &self,
        resource_group_id: &str,
    ) -> Result<Vec<ResourceGroupPeer>> {
        let path = format!("/api/resource-groups/{resource_group_id}/peers");
        self.get_json(&path, "fetch resource group peers").await
    }

    pub async fn report_resource_group_wg_port(
        &self,
        resource_group_id: &str,
        port: u16,
    ) -> Result<()> {
        let path = format!("/api/resource-groups/{resource_group_id}/wg-port");
        let builder = self
            .request(Method::PUT, &path)
            .json(&serde_json::json!({ "port": port }));
        self.send_checked(builder, "report resource group wireguard port")
            .await
            .map(drop)
    }

    pub async fn fetch_gateway_config(
        &self,
        resource_group_id: &str,
        gateway_id: &str,
        source_workload_id: &str,
    ) -> Result<serde_json::Value> {
        let path = format!("/api/resource-groups/{resource_group_id}/gateways/{gateway_id}/config");
        let response = self
            .request(Method::GET, &path)
            .query(&[("source_workload_id", source_workload_id)])
            .send()
            .await
            .context("Failed to fetch ingress gateway config")?;
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(GatewayConfigRejected.into());
        }
        check_status(response, "fetch ingress gateway config")
            .await?
            .json()
            .await
            .context("Failed to parse ingress gateway config response")
    }

    pub async fn fetch_mesh_peers(&self) -> Result<Vec<ResourceGroupPeer>> {
        let known = self
            .mesh_peers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let mut builder = self.request(Method::GET, "/api/agents/mesh-peers");
        if let Some((etag, _)) = &known {
            builder = builder.header(IF_NONE_MATCH, etag);
        }
        let response = builder.send().await.context("Failed to fetch mesh peers")?;
        if let (StatusCode::NOT_MODIFIED, Some((_, peers))) = (response.status(), known) {
            return Ok(peers);
        }
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let peers: Vec<ResourceGroupPeer> = check_status(response, "fetch mesh peers")
            .await?
            .json()
            .await
            .context("Failed to parse fetch mesh peers response")?;
        *self
            .mesh_peers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            etag.map(|etag| (etag, peers.clone()));
        Ok(peers)
    }

    pub async fn fetch_resource_group_vpn_peers(
        &self,
        resource_group_id: &str,
    ) -> Result<Vec<VpnPeer>> {
        let path = format!("/api/resource-groups/{resource_group_id}/vpn-peers");
        self.get_json(&path, "fetch resource group vpn peers").await
    }

    pub async fn fetch_active_resource_group_ids(&self) -> Result<Vec<String>> {
        self.get_json(
            "/api/resource-groups/agent/active-ids",
            "fetch active resource group ids",
        )
        .await
    }

    pub async fn fetch_assigned_workloads(&self) -> Result<Vec<AssignedWorkload>> {
        let items: Vec<serde_json::Value> = self
            .get_json("/api/agents/self/workloads", "fetch workloads")
            .await?;
        parse_workloads(items)
    }

    pub async fn push_workload_stats(&self, stats: Vec<WorkloadStatsUpdate>) -> Result<()> {
        if stats.is_empty() {
            return Ok(());
        }
        let builder = self
            .request(Method::POST, "/api/agents/self/workloads/stats")
            .json(&WorkloadStatsRequest { stats });
        self.send_checked(builder, "push workload stats")
            .await
            .map(drop)
    }

    pub async fn push_logs(&self, entries: Vec<JournalEntry>) -> Result<()> {
        let builder = self
            .request(Method::POST, "/api/agents/self/logs")
            .json(&JournalBatch { entries });
        self.send_checked(builder, "push logs").await.map(drop)
    }

    pub async fn ack_workload_restart(&self, workload_id: &str) -> Result<()> {
        let path = format!("/api/agents/self/workloads/{workload_id}/restart-ack");
        self.send_checked(self.request(Method::POST, &path), "ack workload restart")
            .await
            .map(drop)
    }

    pub async fn fetch_assigned_volumes(&self) -> Result<Vec<AssignedVolume>> {
        let all: Vec<AssignedVolume> = self
            .get_json("/api/agents/self/volumes", "fetch volumes")
            .await?;
        Ok(all.into_iter().filter(|v| v.status == "in_use").collect())
    }
}

async fn check_status(response: Response, action: &str) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    anyhow::bail!("Failed to {action} status={status} {body}")
}

async fn wait_out_throttle(response: &Response) {
    let retry_after = response
        .headers()
        .get("Retry-After")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60);
    let delay = crate::backoff::retry_after_delay(retry_after);
    warn!(
        retry_after,
        delay_ms = delay.as_millis() as u64,
        "Registration rate-limited, retrying"
    );
    tokio::time::sleep(delay).await;
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
        ApiClient::new(url, "pub".to_string(), None, None)
            .unwrap()
            .with_api_key("k".to_string())
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
        let bundle = client.request_master_bundle().await.unwrap();
        assert_eq!(bundle.etcd_name, "etcd1");
        assert_eq!(bundle.vip.unwrap().router_id, 7);
        let vip = client.fetch_cluster_vip().await.unwrap().unwrap();
        assert_eq!(vip.address, "10.0.0.9");
        assert!(client.promote_master().await.unwrap());
    }

    #[tokio::test]
    async fn promotion_conflict_is_not_an_error() {
        let app = Router::new().route(
            "/api/cluster/masters/promote",
            post(|| async { StatusCode::CONFLICT }),
        );
        assert!(!serve(app).await.promote_master().await.unwrap());
    }

    #[tokio::test]
    async fn resource_group_endpoints_parse_responses() {
        let client = serve(gateway()).await;
        let members = client.fetch_resource_group_members("rg").await.unwrap();
        assert_eq!(members[0].agent_id, "rg");
        let services = client
            .fetch_resource_group_site_services("rg")
            .await
            .unwrap();
        assert_eq!(services[0].address, "10.0.0.4");
        let peers = client.fetch_resource_group_peers("rg").await.unwrap();
        assert_eq!(peers[0].wg_endpoint, "1.2.3.4:5");
        assert_eq!(client.fetch_mesh_peers().await.unwrap().len(), 1);
        let config = client.fetch_gateway_config("rg", "gw", "wl").await.unwrap();
        assert!(config["routes"].is_array());
        let vpn = client.fetch_resource_group_vpn_peers("rg").await.unwrap();
        assert_eq!(vpn[0].client_tunnel_ip, "10.8.0.2");
        let ids = client.fetch_active_resource_group_ids().await.unwrap();
        assert_eq!(ids, vec!["rg1".to_string(), "rg2".to_string()]);
    }

    #[tokio::test]
    async fn workload_and_volume_endpoints() {
        let client = serve(gateway()).await;
        let workloads = client.fetch_assigned_workloads().await.unwrap();
        assert_eq!(workloads[0].image, "nginx");
        assert!(!workloads[0].restart_requested);
        assert_eq!(workloads[0].env_vars.as_ref().unwrap()["A"], "b");
        let volumes = client.fetch_assigned_volumes().await.unwrap();
        assert_eq!(volumes.len(), 2);
        assert_eq!(volumes[0].id, "1");
        assert_eq!(volumes[1].id, "2");
        client.ack_workload_restart("w1").await.unwrap();
    }

    #[tokio::test]
    async fn newer_control_plane_fields_and_enum_values_are_tolerated() {
        let mut extended = workload();
        extended["id"] = json!("w-extended");
        extended["new_field"] = json!({"nested": true});
        extended["ports"][0]["new_port_field"] = json!(1);
        let mut unknown_protocol = workload();
        unknown_protocol["id"] = json!("w-unknown-protocol");
        unknown_protocol["ports"] = json!([
            unknown_protocol["ports"][0].clone(),
            {"container_port": 9, "protocol": "sctp"}
        ]);
        let client = serve_workloads(json!([extended, unknown_protocol])).await;
        let workloads = client.fetch_assigned_workloads().await.unwrap();
        let ids: Vec<&str> = workloads.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w-extended", "w-unknown-protocol"]);
        assert_eq!(workloads[1].ports.as_ref().map(Vec::len), Some(1));
    }

    #[tokio::test]
    async fn unreadable_workloads_fail_the_assignment_instead_of_being_dropped() {
        let mut broken = workload();
        broken["id"] = json!("w-broken");
        broken["cpu_millicores"] = json!("many");
        let client = serve_workloads(json!([workload(), broken, 7])).await;
        let error = client.fetch_assigned_workloads().await.unwrap_err();
        assert!(
            error.to_string().contains("ids=w-broken,unknown"),
            "{error}"
        );
    }

    async fn serve_workloads(body: serde_json::Value) -> ApiClient {
        serve(Router::new().route(
            "/api/agents/self/workloads",
            get(move || {
                let body = body.clone();
                async move { Json(body) }
            }),
        ))
        .await
    }

    #[tokio::test]
    async fn requests_carry_the_protocol_version() {
        let client = serve(Router::new().route(
            "/api/agents/self/workloads",
            get(|headers: HeaderMap| async move {
                match headers.get(PROTOCOL_HEADER) {
                    Some(value) if value == "1" => (StatusCode::OK, Json(json!([workload()]))),
                    _ => (StatusCode::UPGRADE_REQUIRED, Json(json!([]))),
                }
            }),
        ))
        .await;
        assert_eq!(client.fetch_assigned_workloads().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn push_endpoints_skip_empty_payloads() {
        let client = serve(gateway()).await;
        client.push_workload_stats(Vec::new()).await.unwrap();
        let stat = WorkloadStatsUpdate {
            workload_id: "w".to_string(),
            cpu_usage_percent: Some(1.0),
            memory_usage_bytes: None,
            network_rx_bytes: None,
            network_tx_bytes: None,
            volume_usage: Vec::new(),
        };
        client.push_workload_stats(vec![stat]).await.unwrap();
        let entry = JournalEntry {
            level: "ERROR".to_string(),
            unit: "u".to_string(),
            message: "m".to_string(),
            timestamp_micros: Some(1),
        };
        client.push_logs(vec![entry]).await.unwrap();
    }

    #[tokio::test]
    async fn every_endpoint_reports_server_errors() {
        let client = failing_client().await;
        assert!(fetch_cluster_ca(client.gateway_url()).await.is_err());
        assert!(client.request_master_bundle().await.is_err());
        assert!(client.fetch_cluster_vip().await.is_err());
        assert!(client.promote_master().await.is_err());
        assert!(client.fetch_resource_group_members("r").await.is_err());
        assert!(client
            .fetch_resource_group_site_services("r")
            .await
            .is_err());
        assert!(client.fetch_resource_group_peers("r").await.is_err());
        assert!(client.fetch_gateway_config("r", "g", "w").await.is_err());
        assert!(client.fetch_mesh_peers().await.is_err());
        assert!(client.fetch_resource_group_vpn_peers("r").await.is_err());
        assert!(client.fetch_active_resource_group_ids().await.is_err());
        assert!(client.fetch_assigned_workloads().await.is_err());
        assert!(client.fetch_assigned_volumes().await.is_err());
        assert!(client.ack_workload_restart("w").await.is_err());
        assert!(client.push_logs(Vec::new()).await.is_err());
        let stat = WorkloadStatsUpdate {
            workload_id: "w".to_string(),
            cpu_usage_percent: None,
            memory_usage_bytes: None,
            network_rx_bytes: None,
            network_tx_bytes: None,
            volume_usage: Vec::new(),
        };
        assert!(client.push_workload_stats(vec![stat]).await.is_err());
    }

    #[tokio::test]
    async fn malformed_bodies_are_parse_errors() {
        let app = Router::new().fallback(|| async { "not json" });
        let client = serve(app).await;
        assert!(client.request_master_bundle().await.is_err());
        assert!(client.fetch_mesh_peers().await.is_err());
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
        let response = client.register(&registration(Some("mid"))).await.unwrap();
        assert_eq!(response.api_key, "secret");
        assert_eq!(response.wg_tunnel_ip.as_deref(), Some("10.9.0.5"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    fn registration(machine_id: Option<&str>) -> RegisterRequest {
        RegisterRequest {
            registration_token: "t".to_string(),
            name: "n".to_string(),
            hostname: "h".to_string(),
            os_type: "linux".to_string(),
            os_version: "1".to_string(),
            architecture: "x86_64".to_string(),
            agent_version: "0.0.0".to_string(),
            tags: None,
            csr_pem: Some("csr".to_string()),
            machine_id: machine_id.map(str::to_string),
        }
    }

    #[tokio::test]
    async fn registration_failures_include_the_body() {
        let client =
            serve(Router::new().fallback(|| async { (StatusCode::FORBIDDEN, "denied") })).await;
        let error = client
            .register(&registration(None))
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
        let statuses = vec![ContainerStatus {
            workload_id: "w".to_string(),
            container_id: "c".to_string(),
            status: "running".to_string(),
        }];
        let response = client
            .heartbeat(
                Uuid::new_v4(),
                Some(statuses),
                Some(crate::system::LiveMetricsCollector::new().sample()),
            )
            .await
            .unwrap();
        assert!(response.ssh_enabled);
        assert_eq!(response.worker_releases["x86_64-linux"].manifest, "m");
        assert_eq!(response.gateway_url.as_deref(), Some("https://gw"));

        let (headers, body) = seen.lock().unwrap().take().unwrap();
        assert_eq!(headers["x-api-key"], "k");
        assert_eq!(headers["x-client-cert"], "line1\\nline2");
        assert_eq!(body["wg_public_key"], "pub");
        assert!(body.get("wg_tunnel_ip").is_none());
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
            .heartbeat(Uuid::nil(), None, None)
            .await
            .unwrap();
        assert!(ok.worker_releases.is_empty());
        assert!(!ok.ssh_enabled);
        let failed = failing_client()
            .await
            .heartbeat(Uuid::nil(), None, None)
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
