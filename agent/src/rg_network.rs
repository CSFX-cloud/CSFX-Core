use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::info;

use crate::rg_dns::RgDnsRegistry;
use crate::spec::{rg_bridge_iface_name, second_host_ip};

const RG_REGISTRY_DIR: &str = "/var/lib/csfx-agent/rg-networks";
pub const S3_SERVICE_NAME: &str = "s3";
pub const PLATFORM_SERVICE_NAME: &str = "csfx";
const S3_DNAT_PORT: u16 = 3900;

pub async fn ensure_bridge(
    resource_group_id: &str,
    cidr: Option<&str>,
    rg_dns_registry: &RgDnsRegistry,
) -> Result<String> {
    let iface = rg_bridge_iface_name(resource_group_id);

    write_registry_entry(resource_group_id).await?;

    if interface_exists(&iface).await? {
        return Ok(iface);
    }

    run_ip(&["link", "add", "name", &iface, "type", "bridge"]).await?;

    if let Some(cidr) = cidr {
        if let Some(gateway) = second_host_ip(cidr) {
            let prefix = cidr.split('/').nth(1).unwrap_or("24");
            run_ip(&[
                "addr",
                "add",
                &format!("{}/{}", gateway, prefix),
                "dev",
                &iface,
            ])
            .await?;

            if let Err(e) = rg_dns_registry
                .upsert(resource_group_id, S3_SERVICE_NAME, &gateway)
                .await
            {
                info!(resource_group_id = %resource_group_id, error = %e, "Failed to register s3 dns record");
            }

            if let Err(e) = rg_dns_registry
                .upsert(resource_group_id, PLATFORM_SERVICE_NAME, &gateway)
                .await
            {
                info!(resource_group_id = %resource_group_id, error = %e, "Failed to register platform dns record");
            }

            if let Err(e) = crate::nftables::dnat_bridge_port(&iface, &gateway, S3_DNAT_PORT).await
            {
                info!(resource_group_id = %resource_group_id, error = %e, "Failed to set up s3 dnat rule");
            }
        }
    }

    run_ip(&["link", "set", "dev", &iface, "up"]).await?;

    crate::nftables::masquerade_bridge_traffic(&iface)
        .await
        .context("Failed to apply nftables masquerade for resource group bridge")?;

    let other_bridges = list_other_bridge_ifaces(resource_group_id).await?;
    crate::nftables::isolate_bridge(&iface, &other_bridges)
        .await
        .context("Failed to apply nftables isolation for resource group bridge")?;

    info!(resource_group_id = %resource_group_id, iface = %iface, "Resource group bridge ready");

    Ok(iface)
}

async fn list_other_bridge_ifaces(exclude_resource_group_id: &str) -> Result<Vec<String>> {
    Ok(list_rg_ids()
        .await?
        .into_iter()
        .filter(|id| id != exclude_resource_group_id)
        .map(|id| rg_bridge_iface_name(&id))
        .collect())
}

pub async fn ensure_mesh_forwarding(resource_group_id: &str, wg_iface: &str) -> Result<()> {
    let iface = rg_bridge_iface_name(resource_group_id);

    tokio::fs::write(format!("/proc/sys/net/ipv4/conf/{}/proxy_arp", iface), b"1")
        .await
        .with_context(|| format!("failed to enable proxy arp iface={}", iface))?;

    crate::nftables::allow_mesh_forwarding(&iface, wg_iface)
        .await
        .context("Failed to exempt mesh traffic from masquerade")
}

pub async fn announce_remote_guest(resource_group_id: &str, guest_ip: &str) -> Result<()> {
    let iface = rg_bridge_iface_name(resource_group_id);
    let address: std::net::Ipv4Addr = guest_ip
        .parse()
        .with_context(|| format!("invalid guest ip {}", guest_ip))?;

    run_ip(&["neigh", "flush", "to", guest_ip, "dev", &iface]).await?;
    crate::arp::announce(&iface, address)
}

pub async fn teardown_bridge(resource_group_id: &str) -> Result<()> {
    let iface = rg_bridge_iface_name(resource_group_id);

    let output = Command::new("ip")
        .args(["link", "delete", &iface])
        .output()
        .await
        .context("failed to execute ip link delete")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("Cannot find device") {
            anyhow::bail!("ip link delete failed iface={} stderr={}", iface, stderr);
        }
    }

    remove_registry_entry(resource_group_id).await;

    info!(resource_group_id = %resource_group_id, iface = %iface, "Resource group bridge removed");

    Ok(())
}

pub async fn list_rg_ids() -> Result<Vec<String>> {
    let mut entries = match tokio::fs::read_dir(RG_REGISTRY_DIR).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("failed to read resource group registry directory"),
    };

    let mut ids = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("failed to read resource group registry entry")?
    {
        if let Some(id) = entry.file_name().to_str() {
            if interface_exists(&rg_bridge_iface_name(id)).await? {
                ids.push(id.to_string());
            }
        }
    }

    Ok(ids)
}

fn registry_entry_path(resource_group_id: &str) -> PathBuf {
    Path::new(RG_REGISTRY_DIR).join(resource_group_id)
}

async fn write_registry_entry(resource_group_id: &str) -> Result<()> {
    tokio::fs::create_dir_all(RG_REGISTRY_DIR)
        .await
        .context("failed to create resource group registry directory")?;
    tokio::fs::write(registry_entry_path(resource_group_id), b"")
        .await
        .context("failed to write resource group registry entry")?;
    Ok(())
}

async fn remove_registry_entry(resource_group_id: &str) {
    let _ = tokio::fs::remove_file(registry_entry_path(resource_group_id)).await;
}

async fn interface_exists(iface: &str) -> Result<bool> {
    let output = Command::new("ip")
        .args(["link", "show", iface])
        .output()
        .await
        .context("failed to execute ip link show")?;

    Ok(output.status.success())
}

async fn run_ip(args: &[&str]) -> Result<()> {
    let output = Command::new("ip")
        .args(args)
        .output()
        .await
        .context("failed to execute ip")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("File exists") {
            return Ok(());
        }
        anyhow::bail!("ip command failed args={:?} stderr={}", args, stderr);
    }

    Ok(())
}
