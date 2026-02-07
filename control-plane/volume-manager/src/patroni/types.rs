use serde::{Deserialize, Serialize};

/// PostgreSQL Node Role in Patroni Cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PostgresNodeRole {
    Primary,
    Replica,
    Standby,
    Unknown,
}

/// Patroni Node Status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatroniNode {
    pub name: String,
    pub role: PostgresNodeRole,
    pub state: PatroniState,
    pub api_url: String,
    pub postgres_url: String,
    pub timeline: Option<u64>,
    pub lag: Option<u64>, // Replication lag in bytes
}

/// Patroni Cluster State
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatroniState {
    Running,
    Starting,
    Stopped,
    Failed,
    Unknown,
}

/// Patroni Cluster Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatroniCluster {
    pub scope: String,
    pub members: Vec<PatroniNode>,
    pub leader: Option<String>,
    pub failover_in_progress: bool,
}

/// Patroni Health Response (from REST API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatroniHealth {
    pub state: String,
    pub role: String,
    pub server_version: Option<u64>,
    pub cluster_unlocked: Option<bool>,
    pub timeline: Option<u64>,
}

/// Patroni Cluster Topology (from /cluster endpoint)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatroniClusterInfo {
    pub members: Vec<PatroniMemberInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatroniMemberInfo {
    pub name: String,
    pub role: String,
    pub state: String,
    pub api_url: String,
    pub host: String,
    pub port: u16,
    pub timeline: Option<u64>,
    pub lag: Option<String>,
}

impl From<&str> for PostgresNodeRole {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "master" | "primary" | "leader" => PostgresNodeRole::Primary,
            "replica" | "standby_leader" => PostgresNodeRole::Replica,
            "standby" => PostgresNodeRole::Standby,
            _ => PostgresNodeRole::Unknown,
        }
    }
}

impl From<&str> for PatroniState {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "running" => PatroniState::Running,
            "starting" => PatroniState::Starting,
            "stopped" => PatroniState::Stopped,
            "failed" => PatroniState::Failed,
            _ => PatroniState::Unknown,
        }
    }
}
