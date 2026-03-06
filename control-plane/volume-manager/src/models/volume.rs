use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VolumeStatus {
    Available,
    Attaching,
    InUse,
    Detaching,
    Deleting,
    Error,
}

impl VolumeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            VolumeStatus::Available => "available",
            VolumeStatus::Attaching => "attaching",
            VolumeStatus::InUse => "in_use",
            VolumeStatus::Detaching => "detaching",
            VolumeStatus::Deleting => "deleting",
            VolumeStatus::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "attaching" => VolumeStatus::Attaching,
            "in_use" => VolumeStatus::InUse,
            "detaching" => VolumeStatus::Detaching,
            "deleting" => VolumeStatus::Deleting,
            "error" => VolumeStatus::Error,
            _ => VolumeStatus::Available,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotStatus {
    Available,
    Creating,
    Deleting,
    Error,
}

impl SnapshotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SnapshotStatus::Available => "available",
            SnapshotStatus::Creating => "creating",
            SnapshotStatus::Deleting => "deleting",
            SnapshotStatus::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVolumeRequest {
    pub name: String,
    pub size_gb: i32,
    pub pool: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachVolumeRequest {
    pub agent_id: Uuid,
    pub workload_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSnapshotRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeResponse {
    pub id: Uuid,
    pub name: String,
    pub size_gb: i32,
    pub pool: String,
    pub image_name: String,
    pub status: VolumeStatus,
    pub attached_to_agent: Option<Uuid>,
    pub attached_to_workload: Option<Uuid>,
    pub mapped_device: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub id: Uuid,
    pub volume_id: Uuid,
    pub name: String,
    pub status: SnapshotStatus,
    pub created_at: DateTime<Utc>,
}
