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

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(WorkloadStatus::Pending),
            "scheduled" => Some(WorkloadStatus::Scheduled),
            "pulling" => Some(WorkloadStatus::Pulling),
            "creating" => Some(WorkloadStatus::Creating),
            "starting" => Some(WorkloadStatus::Starting),
            "running" => Some(WorkloadStatus::Running),
            "failed" => Some(WorkloadStatus::Failed),
            "stopped" => Some(WorkloadStatus::Stopped),
            _ => None,
        }
    }

    pub fn from_str(s: &str) -> Self {
        Self::parse(s).unwrap_or(WorkloadStatus::Pending)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub name: String,
    pub bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeUsage {
    pub mount_path: String,
    pub used_bytes: i64,
    pub total_bytes: i64,
    #[serde(default)]
    pub entries: Vec<UsageEntry>,
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
    #[serde(default)]
    pub volume_mounts: Option<Vec<VolumeMount>>,
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
        crate::services::port_allocator::ensure_requested_in_range(
            self.ports.as_deref().unwrap_or_default(),
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortProtocol {
    #[default]
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub container_port: u16,
    pub protocol: Option<PortProtocol>,
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
    pub guest_ip: Option<String>,
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
    pub volume_usage: Option<Vec<VolumeUsage>>,
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

#[cfg(test)]
mod tests {
    #[test]
    fn port_protocol_rejects_anything_but_tcp_and_udp() {
        for hostile in [
            "tcp; flush ruleset",
            "tcp\ndport 1 accept",
            "TCP",
            "",
            "sctp",
        ] {
            let json = serde_json::json!({ "container_port": 80, "protocol": hostile });
            assert!(serde_json::from_value::<PortMapping>(json).is_err());
        }
        let json = serde_json::json!({ "container_port": 80 });
        assert!(serde_json::from_value::<PortMapping>(json)
            .unwrap()
            .protocol
            .is_none());
    }

    use super::*;

    fn request() -> CreateWorkloadRequest {
        serde_json::from_value(serde_json::json!({
            "name": "web",
            "image": "nginx:latest",
            "cpu_millicores": 100,
            "memory_bytes": 1024,
            "disk_bytes": 2048
        }))
        .unwrap()
    }

    #[test]
    fn validation_rejects_node_ports_outside_the_range() {
        let mut req = request();
        req.ports = serde_json::from_value(serde_json::json!([
            { "container_port": 80, "node_port": 22 }
        ]))
        .unwrap();
        assert!(req.validate().unwrap_err().contains("22"));
    }

    #[test]
    fn workload_status_parsing_rejects_unknown_names() {
        assert_eq!(
            WorkloadStatus::parse("running"),
            Some(WorkloadStatus::Running)
        );
        assert_eq!(
            WorkloadStatus::parse("pending"),
            Some(WorkloadStatus::Pending)
        );
        for hostile in ["", "RUNNING", "running; drop", "ok"] {
            assert!(WorkloadStatus::parse(hostile).is_none(), "{hostile}");
        }
    }

    #[test]
    fn workload_status_round_trips_through_its_name() {
        let statuses = [
            WorkloadStatus::Pending,
            WorkloadStatus::Scheduled,
            WorkloadStatus::Pulling,
            WorkloadStatus::Creating,
            WorkloadStatus::Starting,
            WorkloadStatus::Running,
            WorkloadStatus::Failed,
            WorkloadStatus::Stopped,
        ];
        for status in statuses {
            assert_eq!(WorkloadStatus::from_str(status.as_str()), status);
        }
        assert_eq!(WorkloadStatus::from_str("unknown"), WorkloadStatus::Pending);
    }

    #[test]
    fn restart_policy_round_trips_and_defaults_to_always() {
        for policy in [
            RestartPolicy::Always,
            RestartPolicy::OnFailure,
            RestartPolicy::Never,
        ] {
            assert_eq!(RestartPolicy::from_str(policy.as_str()), policy);
        }
        assert_eq!(RestartPolicy::from_str("sometimes"), RestartPolicy::Always);
    }

    #[test]
    fn runtime_class_round_trips_and_always_needs_kvm() {
        for class in [RuntimeClass::Firecracker, RuntimeClass::Vm] {
            assert_eq!(RuntimeClass::from_str(class.as_str()), class);
            assert!(class.requires_kvm());
        }
        assert_eq!(RuntimeClass::from_str("docker"), RuntimeClass::Firecracker);
    }

    #[test]
    fn desired_state_round_trips_and_defaults_to_running() {
        for state in [DesiredState::Running, DesiredState::Stopped] {
            assert_eq!(DesiredState::from_str(state.as_str()), state);
        }
        assert_eq!(DesiredState::from_str("paused"), DesiredState::Running);
    }

    #[test]
    fn requests_apply_defaults_for_optional_fields() {
        let request = request();
        assert_eq!(request.restart_policy, RestartPolicy::Always);
        assert_eq!(request.runtime_class, RuntimeClass::Firecracker);
        assert!(!request.auto_resources);
        assert!(request.replicas.is_none());
        assert!(request.validate().is_ok());
    }

    #[test]
    fn negative_resources_are_rejected() {
        for mutate in [
            |r: &mut CreateWorkloadRequest| r.cpu_millicores = -1,
            |r: &mut CreateWorkloadRequest| r.memory_bytes = -1,
            |r: &mut CreateWorkloadRequest| r.disk_bytes = -1,
        ] {
            let mut request = request();
            mutate(&mut request);
            assert!(request.validate().unwrap_err().contains("negative"));
        }
    }

    #[test]
    fn limits_below_requests_are_rejected() {
        let mut cpu = request();
        cpu.cpu_limit_millicores = Some(99);
        assert!(cpu.validate().unwrap_err().contains("cpu limit"));

        let mut memory = request();
        memory.memory_limit_bytes = Some(1023);
        assert!(memory.validate().unwrap_err().contains("memory limit"));

        let mut equal = request();
        equal.cpu_limit_millicores = Some(100);
        equal.memory_limit_bytes = Some(1024);
        assert!(equal.validate().is_ok());
    }

    #[test]
    fn replica_counts_are_bounded() {
        for count in [0, MAX_REPLICAS + 1] {
            let mut request = request();
            request.replicas = Some(count);
            assert!(request.validate().unwrap_err().contains("replicas"));
        }
        for count in [1, MAX_REPLICAS] {
            let mut request = request();
            request.replicas = Some(count);
            assert!(request.validate().is_ok());
        }
    }
}
