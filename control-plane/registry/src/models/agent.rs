use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Online,
    Offline,
    Degraded,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Online => write!(f, "online"),
            AgentStatus::Offline => write!(f, "offline"),
            AgentStatus::Degraded => write!(f, "degraded"),
        }
    }
}

impl AgentStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Online" => AgentStatus::Online,
            "Offline" => AgentStatus::Offline,
            "Degraded" => AgentStatus::Degraded,
            _ => AgentStatus::Offline,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreRegisteredAgent {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub expected_os_type: Option<String>,
    pub expected_architecture: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub registration_token: String,
    pub token_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredAgent {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub ip_address: Option<String>,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub status: AgentStatus,
    pub registered_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatistics {
    pub total: usize,
    pub online: usize,
    pub offline: usize,
    pub degraded: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreRegisterRequest {
    pub name: String,
    pub hostname: String,
    pub expected_os_type: Option<String>,
    pub expected_architecture: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub ttl_hours: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreRegisterResponse {
    pub agent_id: Uuid,
    pub registration_token: String,
    pub token_expires_at: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub registration_token: String,
    pub name: String,
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub tags: Option<HashMap<String, String>>,
    pub csr_pem: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub agent_id: Uuid,
    pub api_key: String,
    pub certificate_pem: Option<String>,
    pub ca_cert_pem: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerStatus {
    pub workload_id: String,
    pub container_id: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub status: Option<String>,
    pub container_statuses: Option<Vec<ContainerStatus>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
