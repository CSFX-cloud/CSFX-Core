use anyhow::{Context, Result};
use std::net::Ipv4Addr;
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

    let tenant_range = cidr.map(ensure_tenant_range).transpose()?;

    write_registry_entry(resource_group_id).await?;

    if interface_exists(&iface).await? {
        return Ok(iface);
    }

    if let Some(range) = tenant_range {
        ensure_range_is_unrouted(range).await?;
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

const PRIVATE_RANGES: [(u32, u8); 3] = [(0x0A000000, 8), (0xAC100000, 12), (0xC0A80000, 16)];
const MIN_TENANT_PREFIX: u8 = 16;
const MAX_TENANT_PREFIX: u8 = 29;

fn parse_cidr(cidr: &str) -> Result<(u32, u8)> {
    let (address, prefix) = cidr.split_once('/').context("cidr has no prefix")?;
    let address: Ipv4Addr = address.parse().context("cidr address is invalid")?;
    let prefix: u8 = prefix.parse().context("cidr prefix is invalid")?;
    anyhow::ensure!(prefix <= 32, "cidr prefix is out of range");
    let address = u32::from(address);
    anyhow::ensure!(
        address & prefix_mask(prefix) == address,
        "cidr has host bits set"
    );
    Ok((address, prefix))
}

fn prefix_mask(prefix: u8) -> u32 {
    u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0)
}

fn overlaps(first: (u32, u8), second: (u32, u8)) -> bool {
    let mask = prefix_mask(first.1.min(second.1));
    first.0 & mask == second.0 & mask
}

fn ensure_tenant_range(cidr: &str) -> Result<(u32, u8)> {
    let parsed = parse_cidr(cidr)?;
    let private = PRIVATE_RANGES
        .iter()
        .any(|range| parsed.1 >= range.1 && overlaps(*range, parsed));
    anyhow::ensure!(private, "cidr {cidr} is not inside a private range");
    anyhow::ensure!(
        (MIN_TENANT_PREFIX..=MAX_TENANT_PREFIX).contains(&parsed.1),
        "cidr {cidr} prefix is outside /{MIN_TENANT_PREFIX}../{MAX_TENANT_PREFIX}"
    );
    Ok(parsed)
}

fn routed_destinations(listing: &str) -> Result<Vec<(u32, u8)>> {
    let routes: Vec<serde_json::Value> =
        serde_json::from_str(listing).context("failed to parse ip route listing")?;
    Ok(routes
        .iter()
        .filter_map(|route| route["dst"].as_str())
        .filter(|destination| *destination != "default")
        .filter_map(|destination| {
            let destination = if destination.contains('/') {
                destination.to_string()
            } else {
                format!("{destination}/32")
            };
            parse_cidr(&destination).ok()
        })
        .collect())
}

async fn ensure_range_is_unrouted(range: (u32, u8)) -> Result<()> {
    let output = Command::new("ip")
        .args(["-j", "-4", "route", "show", "table", "all"])
        .output()
        .await
        .context("failed to execute ip")?;
    anyhow::ensure!(output.status.success(), "ip route listing failed");
    let listing = String::from_utf8_lossy(&output.stdout);
    let taken = routed_destinations(&listing)?
        .into_iter()
        .any(|destination| overlaps(destination, range));
    anyhow::ensure!(!taken, "cidr collides with an existing host route");
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeTools;

    #[tokio::test]
    async fn bridges_are_deleted_by_interface_name() {
        let tools = FakeTools::install(&[("ip", "exit 0")]);

        teardown_bridge("rg-1").await.unwrap();

        assert_eq!(
            tools.calls("ip"),
            vec![format!("link delete {}", rg_bridge_iface_name("rg-1"))]
        );
    }

    #[test]
    fn only_private_tenant_ranges_are_accepted() {
        for accepted in ["10.101.0.0/24", "172.16.0.0/16", "192.168.4.0/24"] {
            assert!(ensure_tenant_range(accepted).is_ok(), "{accepted}");
        }
        for rejected in [
            "0.0.0.0/0",
            "8.8.8.0/24",
            "10.0.0.0/8",
            "172.32.0.0/16",
            "192.168.0.0/15",
            "10.1.2.0/30",
            "10.1.2.3/24",
        ] {
            assert!(ensure_tenant_range(rejected).is_err(), "{rejected}");
        }
    }

    #[tokio::test]
    async fn colliding_routes_block_the_bridge_address() {
        let routes = r#"[{"dst":"default"},{"dst":"192.168.1.0/24"},{"dst":"10.100.0.7"}]"#;
        let behaviour = format!("echo '{routes}'");
        let tools = FakeTools::install(&[("ip", behaviour.as_str())]);

        assert!(ensure_range_is_unrouted((0xC0A80100, 24)).await.is_err());
        assert!(ensure_range_is_unrouted((0x0A640000, 16)).await.is_err());
        assert!(ensure_range_is_unrouted((0x0A650000, 24)).await.is_ok());
        assert!(tools
            .calls("ip")
            .iter()
            .all(|call| !call.contains("addr add")));
    }

    #[tokio::test]
    async fn rejected_cidrs_create_no_bridge() {
        let tools = FakeTools::install(&[("ip", "echo '[]'")]);
        let registry = RgDnsRegistry::new();

        let error = ensure_bridge("rg-1", Some("0.0.0.0/0"), &registry).await;

        assert!(error.unwrap_err().to_string().contains("private range"));
        assert!(tools
            .calls("ip")
            .iter()
            .all(|call| !call.contains("link add")));
    }

    #[tokio::test]
    async fn missing_bridges_are_tolerated_on_teardown() {
        {
            let _tools =
                FakeTools::install(&[("ip", "echo 'Cannot find device \"x\"' >&2\nexit 1")]);
            teardown_bridge("rg-1").await.unwrap();
        }
        let _tools = FakeTools::install(&[("ip", "echo 'Operation not permitted' >&2\nexit 1")]);
        let error = teardown_bridge("rg-1").await.unwrap_err().to_string();
        assert!(error.contains("ip link delete failed"));
    }

    #[tokio::test]
    async fn existing_addresses_and_links_are_not_an_error() {
        {
            let _tools =
                FakeTools::install(&[("ip", "echo 'RTNETLINK answers: File exists' >&2\nexit 2")]);
            run_ip(&["link", "add", "name", "br0", "type", "bridge"])
                .await
                .unwrap();
        }
        let _tools = FakeTools::install(&[("ip", "echo 'bad' >&2\nexit 2")]);
        assert!(run_ip(&["link", "add"]).await.is_err());
    }

    #[tokio::test]
    async fn interface_existence_follows_the_exit_status() {
        {
            let _tools = FakeTools::install(&[("ip", "exit 0")]);
            assert!(interface_exists("br0").await.unwrap());
        }
        let _tools = FakeTools::install(&[("ip", "exit 1")]);
        assert!(!interface_exists("br0").await.unwrap());
    }

    #[tokio::test]
    async fn announcing_requires_a_valid_guest_address() {
        let tools = FakeTools::install(&[("ip", "exit 0")]);

        let error = announce_remote_guest("rg-1", "not-an-ip")
            .await
            .unwrap_err();

        assert!(error.to_string().contains("invalid guest ip"));
        assert!(tools.calls("ip").is_empty());
    }

    #[test]
    fn registry_entries_live_below_the_registry_directory() {
        let path = registry_entry_path("rg-1");
        assert_eq!(path, Path::new(RG_REGISTRY_DIR).join("rg-1"));
    }
}
