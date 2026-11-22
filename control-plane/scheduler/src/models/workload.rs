use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::qos::QosClass;
use shared::vm::VmConfig;
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_REPLICAS: u32 = 32;
const MAX_SERVICE_NAME_LENGTH: usize = 63;
const BUCKET_PERMISSIONS: [&str; 2] = ["read", "readwrite"];

pub trait Named: Serialize + serde::de::DeserializeOwned + Default {
    fn name(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    fn parse(name: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(name.to_string())).ok()
    }

    fn from_name(name: &str) -> Self {
        Self::parse(name).unwrap_or_default()
    }
}

impl Named for WorkloadStatus {}
impl Named for RestartPolicy {}
impl Named for RuntimeClass {}
impl Named for TopologySpread {}
impl Named for DesiredState {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkloadStatus {
    #[default]
    Pending,
    Scheduled,
    Pulling,
    Creating,
    Starting,
    Running,
    Failed,
    Stopped,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    #[default]
    Always,
    OnFailure,
    Never,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeClass {
    #[default]
    Firecracker,
    Vm,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TopologySpread {
    #[default]
    None,
    Soft,
    Hard,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DesiredState {
    #[default]
    Running,
    Stopped,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateWorkloadRequest {
    pub image: Option<String>,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub restart_policy: Option<RestartPolicy>,
    pub max_restarts: Option<i32>,
    #[serde(default)]
    pub volume_mounts: Option<Vec<VolumeMount>>,
    pub cpu_millicores: Option<i32>,
    pub memory_bytes: Option<i64>,
    pub disk_bytes: Option<i64>,
    pub vm_config: Option<VmConfig>,
    #[serde(skip)]
    pub guest_ip: Option<String>,
}

impl UpdateWorkloadRequest {
    pub fn touches_vm(&self) -> bool {
        self.cpu_millicores.is_some()
            || self.memory_bytes.is_some()
            || self.disk_bytes.is_some()
            || self.vm_config.is_some()
    }
}

pub fn is_valid_service_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let edge = |byte: &u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    !bytes.is_empty()
        && bytes.len() <= MAX_SERVICE_NAME_LENGTH
        && bytes.first().is_some_and(edge)
        && bytes.last().is_some_and(edge)
        && bytes.iter().all(|byte| edge(byte) || *byte == b'-')
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
    #[serde(default)]
    pub restart_policy: RestartPolicy,
    #[serde(default)]
    pub max_restarts: Option<i32>,
    #[serde(default)]
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
    #[serde(default)]
    pub topology_spread: TopologySpread,
    #[serde(default)]
    pub vm_config: Option<VmConfig>,
    #[serde(skip)]
    pub replaces: Option<Uuid>,
}

impl CreateWorkloadRequest {
    fn validate_vm_config(&self) -> Result<(), String> {
        let Some(config) = &self.vm_config else {
            return Ok(());
        };
        if self.runtime_class != RuntimeClass::Vm {
            return Err("vm config requires the vm runtime class".to_string());
        }
        config.validate()
    }

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
        self.validate_vm_config()?;
        self.validate_bucket_permissions()?;
        if self
            .service_name
            .as_deref()
            .is_some_and(|name| !is_valid_service_name(name))
        {
            return Err("service name must be a lowercase dns label".to_string());
        }
        crate::services::port_allocator::ensure_requested_in_range(
            self.ports.as_deref().unwrap_or_default(),
        )
    }

    fn validate_bucket_permissions(&self) -> Result<(), String> {
        let invalid = self
            .bucket_bindings
            .iter()
            .flatten()
            .filter_map(|binding| binding.permissions.as_deref())
            .any(|permissions| !BUCKET_PERMISSIONS.contains(&permissions));
        if invalid {
            return Err("bucket permissions must be read or readwrite".to_string());
        }
        Ok(())
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

pub fn node_ports_of(ports: &serde_json::Value) -> Vec<u16> {
    serde_json::from_value::<Vec<PortMapping>>(ports.clone())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|mapping| mapping.node_port)
        .collect()
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
    pub replaces_workload_id: Option<Uuid>,
    pub retire_after: Option<DateTime<Utc>>,
    pub rollout_error: Option<String>,
    pub service_name: Option<String>,
    pub restart_policy: RestartPolicy,
    pub max_restarts: Option<i32>,
    pub restart_count: i32,
    pub runtime_class: RuntimeClass,
    pub desired_state: DesiredState,
    pub vm_config: Option<VmConfig>,
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
    pub zone: String,
    pub spread_counts: HashMap<String, u32>,
    pub used_node_ports: HashMap<u16, Uuid>,
    pub architecture: String,
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
    fn enum_names_round_trip_and_reject_unknown_values() {
        assert_eq!(
            WorkloadStatus::parse("running"),
            Some(WorkloadStatus::Running)
        );
        for hostile in ["", "RUNNING", "running; drop", "ok"] {
            assert!(WorkloadStatus::parse(hostile).is_none(), "{hostile}");
        }
        assert_eq!(RestartPolicy::OnFailure.name(), "on-failure");
        assert_eq!(
            RestartPolicy::from_name("on-failure"),
            RestartPolicy::OnFailure
        );
        assert_eq!(
            serde_json::from_str::<RestartPolicy>("\"on-failure\"").unwrap(),
            RestartPolicy::OnFailure
        );
        assert_eq!(DesiredState::Stopped.name(), "stopped");
        assert_eq!(RuntimeClass::Vm.name(), "vm");
        assert_eq!(TopologySpread::Hard.name(), "hard");
    }

    #[test]
    fn unknown_stored_names_fall_back_to_the_default() {
        assert_eq!(
            WorkloadStatus::from_name("unknown"),
            WorkloadStatus::Pending
        );
        assert_eq!(RestartPolicy::from_name("sometimes"), RestartPolicy::Always);
        assert_eq!(RuntimeClass::from_name("docker"), RuntimeClass::Firecracker);
        assert_eq!(DesiredState::from_name("paused"), DesiredState::Running);
        assert_eq!(TopologySpread::from_name("loose"), TopologySpread::None);
    }

    #[test]
    fn validation_limits_bucket_permissions_to_read_and_readwrite() {
        let binding = |permissions: Option<&str>| {
            serde_json::json!([{
                "bucket_id": Uuid::new_v4(),
                "permissions": permissions
            }])
        };
        for allowed in [None, Some("read"), Some("readwrite")] {
            let mut req = request();
            req.bucket_bindings = serde_json::from_value(binding(allowed)).unwrap();
            assert!(req.validate().is_ok());
        }
        for denied in ["owner", "write", ""] {
            let mut req = request();
            req.bucket_bindings = serde_json::from_value(binding(Some(denied))).unwrap();
            assert!(req.validate().is_err());
        }
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
