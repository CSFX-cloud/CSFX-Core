use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::qos::QosClass;
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_REPLICAS: u32 = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkloadStatus {
    Pending,
    Scheduled,
    Pulling,
    Creating,
    Starting,
    Running,
    Failed,
    Stopped,
}

impl WorkloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkloadStatus::Pending => "pending",
            WorkloadStatus::Scheduled => "scheduled",
            WorkloadStatus::Pulling => "pulling",
            WorkloadStatus::Creating => "creating",
            WorkloadStatus::Starting => "starting",
            WorkloadStatus::Running => "running",
            WorkloadStatus::Failed => "failed",
            WorkloadStatus::Stopped => "stopped",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "scheduled" => WorkloadStatus::Scheduled,
            "pulling" => WorkloadStatus::Pulling,
            "creating" => WorkloadStatus::Creating,
            "starting" => WorkloadStatus::Starting,
            "running" => WorkloadStatus::Running,
            "failed" => WorkloadStatus::Failed,
            "stopped" => WorkloadStatus::Stopped,
            _ => WorkloadStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    pub volume_id: Uuid,
    pub mount_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketBinding {
    pub bucket_id: Uuid,
    #[serde(default)]
    pub permissions: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

impl RestartPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            RestartPolicy::Always => "always",
            RestartPolicy::OnFailure => "on-failure",
            RestartPolicy::Never => "never",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "on-failure" => RestartPolicy::OnFailure,
            "never" => RestartPolicy::Never,
            _ => RestartPolicy::Always,
        }
    }
}

fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::Always
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeClass {
    Firecracker,
    Vm,
}

impl RuntimeClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeClass::Firecracker => "firecracker",
            RuntimeClass::Vm => "vm",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "vm" => RuntimeClass::Vm,
            _ => RuntimeClass::Firecracker,
        }
    }

    pub fn requires_kvm(&self) -> bool {
        true
    }
}

fn default_runtime_class() -> RuntimeClass {
    RuntimeClass::Firecracker
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DesiredState {
    Running,
    Stopped,
}

impl DesiredState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DesiredState::Running => "running",
            DesiredState::Stopped => "stopped",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "stopped" => DesiredState::Stopped,
            _ => DesiredState::Running,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkloadRequest {
    pub image: Option<String>,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub restart_policy: Option<RestartPolicy>,
    pub max_restarts: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkloadRequest {
    pub name: String,
    pub image: String,
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    pub disk_bytes: i64,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub volume_mounts: Option<Vec<VolumeMount>>,
    #[serde(default)]
    pub bucket_bindings: Option<Vec<BucketBinding>>,
    pub resource_group_id: Option<Uuid>,
    #[serde(default)]
    pub stack_id: Option<Uuid>,
    #[serde(default)]
    pub service_name: Option<String>,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: RestartPolicy,
    #[serde(default)]
    pub max_restarts: Option<i32>,
    #[serde(default = "default_runtime_class")]
    pub runtime_class: RuntimeClass,
    #[serde(default)]
    pub cpu_limit_millicores: Option<i32>,
    #[serde(default)]
    pub memory_limit_bytes: Option<i64>,
    #[serde(default)]
    pub auto_resources: bool,
    #[serde(default)]
    pub replicas: Option<u32>,
    #[serde(default)]
    pub spread_key: Option<String>,
}

impl CreateWorkloadRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.cpu_millicores < 0 || self.memory_bytes < 0 || self.disk_bytes < 0 {
            return Err("resource requests must not be negative".to_string());
        }
        if self
            .cpu_limit_millicores
            .is_some_and(|limit| limit < self.cpu_millicores)
        {
            return Err("cpu limit must not be below the cpu request".to_string());
        }
        if self
            .memory_limit_bytes
            .is_some_and(|limit| limit < self.memory_bytes)
        {
            return Err("memory limit must not be below the memory request".to_string());
        }
        if self
            .replicas
            .is_some_and(|count| !(1..=MAX_REPLICAS).contains(&count))
        {
            return Err(format!("replicas must be between 1 and {}", MAX_REPLICAS));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub container_port: u16,
    pub protocol: Option<String>,
    pub rg_port: Option<u16>,
    pub node_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkloadResponse {
    pub workload_id: Uuid,
    pub status: WorkloadStatus,
    pub assigned_agent_id: Option<Uuid>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadResponse {
    pub id: Uuid,
    pub name: String,
    pub image: String,
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    pub disk_bytes: i64,
    pub cpu_limit_millicores: Option<i32>,
    pub memory_limit_bytes: Option<i64>,
    pub qos_class: QosClass,
    pub auto_resources: bool,
    pub recommended_cpu_millicores: Option<i32>,
    pub recommended_memory_bytes: Option<i64>,
    pub status: WorkloadStatus,
    pub assigned_agent_id: Option<Uuid>,
    pub container_id: Option<String>,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub volume_mounts: Option<Vec<VolumeMount>>,
    pub resource_group_id: Option<Uuid>,
    pub stack_id: Option<Uuid>,
    pub service_name: Option<String>,
    pub restart_policy: RestartPolicy,
    pub max_restarts: Option<i32>,
    pub restart_count: i32,
    pub runtime_class: RuntimeClass,
    pub desired_state: DesiredState,
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_bytes: Option<i64>,
    pub network_rx_bytes: Option<i64>,
    pub network_tx_bytes: Option<i64>,
    pub stats_updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AgentResources {
    pub agent_id: Uuid,
    pub allocatable_cpu_millicores: i32,
    pub allocatable_memory_bytes: i64,
    pub free_cpu_millicores: i32,
    pub free_memory_bytes: i64,
    pub free_disk_bytes: i64,
    pub kvm_capable: bool,
    pub spread_counts: HashMap<String, u32>,
}
