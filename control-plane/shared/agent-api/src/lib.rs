use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub registration_token: String,
    pub name: String,
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
    #[serde(default)]
    pub csr_pem: Option<String>,
    #[serde(default)]
    pub machine_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub agent_id: Uuid,
    pub api_key: String,
    pub certificate_pem: Option<String>,
    pub ca_cert_pem: Option<String>,
    pub wg_tunnel_ip: Option<String>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    pub workload_id: String,
    pub container_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    #[serde(default)]
    pub container_statuses: Option<Vec<ContainerStatus>>,
    #[serde(flatten, default)]
    pub metrics: Option<SystemMetrics>,
    #[serde(default)]
    pub wg_public_key: Option<String>,
    #[serde(default)]
    pub wg_endpoint: Option<String>,
    #[serde(default)]
    pub agent_version: Option<String>,
    #[serde(default)]
    pub release_version: Option<String>,
    #[serde(default)]
    pub kvm_capable: bool,
    #[serde(default)]
    pub ssh_host_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRelease {
    pub manifest: String,
    pub signature: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub worker_releases: HashMap<String, SignedRelease>,
    #[serde(default)]
    pub ssh_enabled: bool,
    #[serde(default)]
    pub gateway_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metrics_are_flattened_into_the_heartbeat_body() {
        let body = json!({
            "cpu_usage_percent": 1.5, "cpu_cores": 4,
            "memory_total_bytes": 8, "memory_used_bytes": 4,
            "disk_total_bytes": 8, "disk_used_bytes": 4,
            "network_rx_bytes": 1, "network_tx_bytes": 2, "uptime_seconds": 3,
            "kvm_capable": true
        });
        let request: HeartbeatRequest = serde_json::from_value(body).unwrap();
        assert_eq!(request.metrics.unwrap().cpu_cores, 4);
        assert!(request.kvm_capable);
        let bare: HeartbeatRequest = serde_json::from_value(json!({})).unwrap();
        assert!(bare.metrics.is_none());
        assert!(serde_json::to_value(bare)
            .unwrap()
            .get("cpu_cores")
            .is_none());
    }
}
