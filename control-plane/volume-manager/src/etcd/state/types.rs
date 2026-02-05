use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Volume State in etcd
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeState {
    pub id: Uuid,
    pub name: String,
    pub size_gb: u64,
    pub pool: String,
    pub status: VolumeStatus,
    pub encrypted: bool,
    pub node_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VolumeStatus {
    Creating,
    Available,
    InUse,
    Deleting,
    Error,
    Migrating,
}

impl VolumeState {
    pub fn new(name: String, size_gb: u64, pool: String, encrypted: bool) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            size_gb,
            pool,
            status: VolumeStatus::Creating,
            encrypted,
            node_id: None,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    pub fn update_status(&mut self, status: VolumeStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        self.version += 1;
    }
}

/// Node State für Failover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub node_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub status: NodeStatus,
    pub role: NodeRole,
    pub last_heartbeat: DateTime<Utc>,
    pub volumes: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    Online,
    Offline,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeRole {
    Leader,
    Follower,
}

/// Snapshot State
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotState {
    pub id: Uuid,
    pub volume_id: Uuid,
    pub name: String,
    pub size_gb: u64,
    pub status: SnapshotStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SnapshotStatus {
    Creating,
    Available,
    Deleting,
    Error,
}
