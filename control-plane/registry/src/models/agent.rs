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
            AgentStatus::Online => write!(f, "Online"),
            AgentStatus::Offline => write!(f, "Offline"),
            AgentStatus::Degraded => write!(f, "Degraded"),
        }
    }
}

impl AgentStatus {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "online" => AgentStatus::Online,
            "degraded" => AgentStatus::Degraded,
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
    pub wg_tunnel_ip: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
pub struct ErrorResponse {
    pub error: String,
}

impl From<entity::agents::Model> for RegisteredAgent {
    fn from(agent: entity::agents::Model) -> Self {
        Self {
            id: agent.id,
            name: agent.name,
            hostname: agent.hostname,
            ip_address: agent.ip_address,
            os_type: agent.os_type,
            os_version: agent.os_version,
            architecture: agent.architecture,
            agent_version: agent.agent_version,
            status: AgentStatus::from_str(&agent.status),
            registered_at: agent.registered_at.and_utc(),
            last_heartbeat: agent.last_heartbeat.map(|time| time.and_utc()),
            tags: agent
                .tags
                .and_then(|tags| serde_json::from_value(tags).ok()),
            wg_tunnel_ip: agent.wg_tunnel_ip,
        }
    }
}
