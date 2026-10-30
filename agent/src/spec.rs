use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub container_port: u16,
    pub protocol: Option<String>,
    pub rg_port: Option<u16>,
    pub node_port: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct VolumeMount {
    pub volume_id: String,
    pub mount_path: String,
    pub device_path: String,
}

#[derive(Debug, Clone)]
pub struct WorkloadSpec {
    pub workload_id: String,
    pub image: String,
    pub cpu_millicores: i32,
    pub cpu_limit_millicores: i32,
    pub memory_bytes: i64,
    pub memory_limit_bytes: i64,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub volume_mounts: Option<Vec<VolumeMount>>,
    pub service_name: Option<String>,
    pub resource_group_id: Option<String>,
    pub resource_group_cidr: Option<String>,
    pub guest_ip: Option<String>,
    pub vm_spec: Option<VmSpec>,
}

const MIN_CPU_MILLICORES: i32 = 100;
const CPU_PERIOD_MICROS: i64 = 100_000;
const MILLICORES_PER_CORE: f64 = 1000.0;
const MILLICORES_PER_PERCENT: i32 = 10;
const MAX_CPU_WEIGHT: i32 = 10_000;

impl WorkloadSpec {
    pub fn vcpu_count(&self) -> i64 {
        (self.cpu_limit_millicores.max(MIN_CPU_MILLICORES) as f64 / MILLICORES_PER_CORE).ceil()
            as i64
    }

    pub fn cpu_weight(&self) -> i32 {
        (self.cpu_millicores / MILLICORES_PER_PERCENT).clamp(1, MAX_CPU_WEIGHT)
    }

    pub fn cpu_quota_micros(&self) -> i64 {
        self.cpu_limit_millicores.max(MIN_CPU_MILLICORES) as i64 * CPU_PERIOD_MICROS / 1000
    }

    pub fn cpu_quota_percent(&self) -> i32 {
        self.cpu_limit_millicores.max(MIN_CPU_MILLICORES) / MILLICORES_PER_PERCENT
    }

    pub fn cpu_max(&self) -> String {
        format!("{} {}", self.cpu_quota_micros(), CPU_PERIOD_MICROS)
    }

    pub fn balloon_bytes(&self) -> i64 {
        (self.memory_limit_bytes - self.memory_bytes).max(0)
    }
}

#[derive(Debug, Clone)]
pub struct VmSpec {
    pub boot_disk_path: String,
    pub boot_disk_size_bytes: i64,
    pub iso_url: Option<String>,
}

pub fn rg_network_name(resource_group_id: &str) -> String {
    format!("csfx-rg-{}", resource_group_id)
}

pub fn second_host_ip(cidr: &str) -> Option<String> {
    nth_host_ip(cidr, 2)
}

pub fn nth_host_ip(cidr: &str, offset: u32) -> Option<String> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let octets: Vec<u8> = parts[0].split('.').filter_map(|o| o.parse().ok()).collect();
    if octets.len() != 4 {
        return None;
    }
    let n = u32::from_be_bytes([octets[0], octets[1], octets[2], octets[3]]);
    let host = n + offset;
    let [a, b, c, d] = host.to_be_bytes();
    Some(format!("{}.{}.{}.{}", a, b, c, d))
}

pub fn cidr_host_count(cidr: &str) -> Option<u32> {
    let prefix: u32 = cidr.split('/').nth(1)?.parse().ok()?;
    if prefix > 32 {
        return None;
    }
    Some(1u32 << (32 - prefix))
}

pub fn rg_wireguard_port(resource_group_id: &str) -> u16 {
    const FNV_OFFSET_BASIS: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in resource_group_id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    49152u16 + (hash % (65535 - 49152)) as u16
}

pub fn rg_bridge_iface_name(resource_group_id: &str) -> String {
    const FNV_OFFSET_BASIS: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in resource_group_id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("csfxrg{:08x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: i64 = 1024 * 1024;

    fn spec(cpu: i32, cpu_limit: i32, memory_mib: i64, memory_limit_mib: i64) -> WorkloadSpec {
        WorkloadSpec {
            workload_id: "w".to_string(),
            image: "i".to_string(),
            cpu_millicores: cpu,
            cpu_limit_millicores: cpu_limit,
            memory_bytes: memory_mib * MIB,
            memory_limit_bytes: memory_limit_mib * MIB,
            env_vars: None,
            ports: None,
            volume_mounts: None,
            service_name: None,
            resource_group_id: None,
            resource_group_cidr: None,
            guest_ip: None,
            vm_spec: None,
        }
    }

    #[test]
    fn fixed_workload_is_capped_at_its_request_without_balloon() {
        let fixed = spec(1000, 1000, 512, 512);
        assert_eq!(fixed.vcpu_count(), 1);
        assert_eq!(fixed.cpu_weight(), 100);
        assert_eq!(fixed.cpu_max(), "100000 100000");
        assert_eq!(fixed.cpu_quota_percent(), 100);
        assert_eq!(fixed.balloon_bytes(), 0);
    }

    #[test]
    fn burstable_workload_gets_limit_sized_vm_and_request_sized_weight() {
        let burst = spec(500, 2500, 256, 1024);
        assert_eq!(burst.vcpu_count(), 3);
        assert_eq!(burst.cpu_weight(), 50);
        assert_eq!(burst.cpu_max(), "250000 100000");
        assert_eq!(burst.balloon_bytes(), 768 * MIB);
    }

    #[test]
    fn best_effort_workload_keeps_minimum_weight_and_quota() {
        let idle = spec(0, 0, 0, 0);
        assert_eq!(idle.cpu_weight(), 1);
        assert_eq!(idle.vcpu_count(), 1);
        assert_eq!(idle.cpu_max(), "10000 100000");
    }
}
