use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::spec::{cidr_host_count, nth_host_ip, second_host_ip};

const DHCP_DIR: &str = "/var/lib/csfx-agent/dhcp";
const FIRST_GUEST_OFFSET: u32 = 3;

fn leases_file_path(resource_group_id: &str) -> PathBuf {
    PathBuf::from(DHCP_DIR).join(format!("{}.leases", resource_group_id))
}

fn hosts_file_path(resource_group_id: &str) -> PathBuf {
    PathBuf::from(DHCP_DIR).join(format!("{}.hosts", resource_group_id))
}

fn pid_file_path(resource_group_id: &str) -> PathBuf {
    PathBuf::from(DHCP_DIR).join(format!("{}.pid", resource_group_id))
}

pub fn mac_address_for_workload(workload_id: &str) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in workload_id.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    let bytes = hash.to_be_bytes();
    format!(
        "52:54:{:02x}:{:02x}:{:02x}:{:02x}",
        bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

pub struct RgDhcpSupervisor {
    processes: Mutex<HashMap<String, Child>>,
    reservations: Mutex<HashMap<String, HashMap<String, (String, String)>>>,
}

impl Default for RgDhcpSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl RgDhcpSupervisor {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            reservations: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_reservation(
        &self,
        resource_group_id: &str,
        cidr: &str,
        bridge_iface: &str,
        workload_id: &str,
        mac_address: &str,
        guest_ip: &str,
    ) -> Result<()> {
        {
            let mut reservations = self.reservations.lock().await;
            let rg_reservations = reservations
                .entry(resource_group_id.to_string())
                .or_default();
            rg_reservations.insert(
                workload_id.to_string(),
                (mac_address.to_string(), guest_ip.to_string()),
            );
        }

        self.write_hosts_file(resource_group_id).await?;
        self.ensure_running(resource_group_id, cidr, bridge_iface)
            .await
    }

    pub async fn remove_reservation(&self, resource_group_id: &str, workload_id: &str) {
        {
            let mut reservations = self.reservations.lock().await;
            if let Some(rg_reservations) = reservations.get_mut(resource_group_id) {
                rg_reservations.remove(workload_id);
            }
        }

        if let Err(e) = self.write_hosts_file(resource_group_id).await {
            warn!(resource_group_id = %resource_group_id, error = %e, "Failed to rewrite dhcp hosts file");
        }
    }

    async fn write_hosts_file(&self, resource_group_id: &str) -> Result<()> {
        fs::create_dir_all(DHCP_DIR)
            .await
            .context("Failed to create dhcp directory")?;

        let reservations = self.reservations.lock().await;
        let empty = HashMap::new();
        let rg_reservations = reservations.get(resource_group_id).unwrap_or(&empty);

        let mut contents = String::new();
        for (mac_address, guest_ip) in rg_reservations.values() {
            contents.push_str(&format!("{},{}\n", mac_address, guest_ip));
        }

        fs::write(hosts_file_path(resource_group_id), contents)
            .await
            .context("Failed to write dhcp hosts file")?;

        Ok(())
    }

    async fn ensure_running(
        &self,
        resource_group_id: &str,
        cidr: &str,
        bridge_iface: &str,
    ) -> Result<()> {
        let mut processes = self.processes.lock().await;
        if let Some(child) = processes.get_mut(resource_group_id) {
            if is_alive(child).await {
                return Ok(());
            }
            info!(resource_group_id = %resource_group_id, "Resource group dhcp process exited, respawning");
        }

        let (range_start, range_end) =
            dhcp_range(cidr).with_context(|| format!("invalid resource group cidr {}", cidr))?;
        let gateway = second_host_ip(cidr)
            .with_context(|| format!("invalid resource group cidr {}", cidr))?;

        fs::create_dir_all(DHCP_DIR)
            .await
            .context("Failed to create dhcp directory")?;

        let child = Command::new("dnsmasq")
            .arg("--keep-in-foreground")
            .arg("--no-daemon")
            .arg("--port=0")
            .arg("--no-resolv")
            .arg("--no-hosts")
            .arg(format!("--interface={}", bridge_iface))
            .arg("--bind-interfaces")
            .arg(format!("--dhcp-range={},{},12h", range_start, range_end))
            .arg(format!("--dhcp-option=option:router,{}", gateway))
            .arg(format!(
                "--dhcp-hostsfile={}",
                hosts_file_path(resource_group_id).display()
            ))
            .arg(format!(
                "--dhcp-leasefile={}",
                leases_file_path(resource_group_id).display()
            ))
            .arg(format!(
                "--pid-file={}",
                pid_file_path(resource_group_id).display()
            ))
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn dnsmasq process")?;

        info!(resource_group_id = %resource_group_id, bridge_iface = %bridge_iface, "Resource group dhcp process started");

        processes.insert(resource_group_id.to_string(), child);

        Ok(())
    }

    pub async fn check_liveness(&self) {
        let mut processes = self.processes.lock().await;
        let mut dead = Vec::new();

        for (resource_group_id, child) in processes.iter_mut() {
            if !is_alive(child).await {
                dead.push(resource_group_id.clone());
            }
        }

        for resource_group_id in dead {
            warn!(resource_group_id = %resource_group_id, "Resource group dhcp process died, will respawn on next reconcile");
            processes.remove(&resource_group_id);
        }
    }

    pub async fn stop(&self, resource_group_id: &str) {
        let mut processes = self.processes.lock().await;
        if let Some(mut child) = processes.remove(resource_group_id) {
            let _ = child.kill().await;
        }

        self.reservations.lock().await.remove(resource_group_id);

        let _ = fs::remove_file(hosts_file_path(resource_group_id)).await;
        let _ = fs::remove_file(leases_file_path(resource_group_id)).await;
        let _ = fs::remove_file(pid_file_path(resource_group_id)).await;
    }
}

fn dhcp_range(cidr: &str) -> Option<(String, String)> {
    let host_count = cidr_host_count(cidr)?;
    let usable_hosts = host_count.saturating_sub(FIRST_GUEST_OFFSET + 1);
    let start = nth_host_ip(cidr, FIRST_GUEST_OFFSET)?;
    let end = nth_host_ip(cidr, FIRST_GUEST_OFFSET + usable_hosts.saturating_sub(1))?;
    Some((start, end))
}

async fn is_alive(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_address_is_deterministic() {
        let a = mac_address_for_workload("workload-1");
        let b = mac_address_for_workload("workload-1");
        let c = mac_address_for_workload("workload-2");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("52:54:"));
    }
}
