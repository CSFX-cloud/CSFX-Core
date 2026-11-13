use anyhow::{Context, Result};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{info, warn};

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
    ensure_bridge_in(
        Path::new(RG_REGISTRY_DIR),
        resource_group_id,
        cidr,
        rg_dns_registry,
    )
    .await
}

async fn ensure_bridge_in(
    registry_dir: &Path,
    resource_group_id: &str,
    cidr: Option<&str>,
    rg_dns_registry: &RgDnsRegistry,
) -> Result<String> {
    let iface = rg_bridge_iface_name(resource_group_id);

    let tenant_range = cidr.map(ensure_tenant_range).transpose()?;

    write_registry_entry(registry_dir, resource_group_id, cidr).await?;

    let exists = interface_exists(&iface).await?;
    if let Some(range) = tenant_range {
        if !exists || !has_ipv4_address(&iface).await? {
            ensure_range_is_unrouted(range).await?;
        }
    }
    if !exists {
        run_ip(&["link", "add", "name", &iface, "type", "bridge"]).await?;
    }

    if let Some(gateway) = cidr.and_then(second_host_ip) {
        register_gateway_records(resource_group_id, &gateway, rg_dns_registry).await;
    }

    reconcile_in(registry_dir)
        .await
        .context("Failed to reconcile resource group bridges")?;

    info!(resource_group_id = %resource_group_id, iface = %iface, "Resource group bridge ready");

    Ok(iface)
}

pub async fn record_cidrs(groups: &[(String, String)]) {
    record_cidrs_in(Path::new(RG_REGISTRY_DIR), groups).await;
}

async fn record_cidrs_in(registry_dir: &Path, groups: &[(String, String)]) {
    for (resource_group_id, cidr) in groups {
        let path = registry_entry_path(registry_dir, resource_group_id);
        if tokio::fs::read_to_string(&path).await.ok().as_deref() == Some(cidr.as_str()) {
            continue;
        }
        if let Err(e) = write_registry_entry(registry_dir, resource_group_id, Some(cidr)).await {
            warn!(resource_group_id = %resource_group_id, error = %e, "Failed to record resource group range");
        }
    }
}

async fn register_gateway_records(
    resource_group_id: &str,
    gateway: &str,
    rg_dns_registry: &RgDnsRegistry,
) {
    for service_name in [S3_SERVICE_NAME, PLATFORM_SERVICE_NAME] {
        if let Err(e) = rg_dns_registry
            .upsert(resource_group_id, service_name, gateway)
            .await
        {
            info!(resource_group_id = %resource_group_id, service = service_name, error = %e, "Failed to register dns record");
        }
    }
}

struct Bridge {
    resource_group_id: String,
    iface: String,
    cidr: Option<String>,
}

impl Bridge {
    fn gateway(&self) -> Option<(String, String)> {
        let cidr = self.cidr.as_deref()?;
        let gateway = second_host_ip(cidr)?;
        let prefix = cidr.split('/').nth(1).unwrap_or("24").to_string();
        Some((gateway, prefix))
    }

    fn rules(&self) -> crate::nftables::BridgeRules {
        crate::nftables::BridgeRules {
            name: self.iface.clone(),
            mesh_iface: self
                .cidr
                .as_ref()
                .map(|_| crate::wireguard::rg_interface_name(&self.resource_group_id)),
        }
    }
}

pub async fn reconcile_bridges() -> Result<()> {
    reconcile_in(Path::new(RG_REGISTRY_DIR)).await
}

async fn reconcile_in(registry_dir: &Path) -> Result<()> {
    let bridges = list_bridges(registry_dir).await?;
    if bridges.is_empty() {
        return Ok(());
    }

    let mut ready = Vec::new();
    for bridge in &bridges {
        match repair_address(bridge).await {
            Ok(()) => ready.push(bridge),
            Err(e) => {
                warn!(iface = %bridge.iface, error = %e, "Failed to repair bridge address, leaving bridge down");
            }
        }
    }

    let rules: Vec<_> = bridges.iter().map(Bridge::rules).collect();
    crate::nftables::apply_bridge_rules(&rules)
        .await
        .context("Failed to apply nftables rules for resource group bridges")?;

    for bridge in ready {
        if let Some((gateway, _)) = bridge.gateway() {
            if let Err(e) =
                crate::nftables::ensure_bridge_dnat(&bridge.iface, &gateway, S3_DNAT_PORT).await
            {
                info!(iface = %bridge.iface, error = %e, "Failed to set up s3 dnat rule");
            }
        }
        enable_proxy_arp(bridge).await?;
        run_ip(&["link", "set", "dev", &bridge.iface, "up"]).await?;
    }

    Ok(())
}

async fn repair_address(bridge: &Bridge) -> Result<()> {
    let Some((gateway, prefix)) = bridge.gateway() else {
        return Ok(());
    };
    run_ip(&[
        "addr",
        "add",
        &format!("{gateway}/{prefix}"),
        "dev",
        &bridge.iface,
    ])
    .await
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

async fn enable_proxy_arp(bridge: &Bridge) -> Result<()> {
    if bridge.cidr.is_none() {
        return Ok(());
    }
    let path = format!("/proc/sys/net/ipv4/conf/{}/proxy_arp", bridge.iface);
    match tokio::fs::write(&path, b"1").await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => {
            result.with_context(|| format!("failed to enable proxy arp iface={}", bridge.iface))
        }
    }
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
    teardown_in(Path::new(RG_REGISTRY_DIR), resource_group_id).await
}

async fn teardown_in(registry_dir: &Path, resource_group_id: &str) -> Result<()> {
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

    remove_registry_entry(registry_dir, resource_group_id).await;
    crate::nftables::remove_bridge_dnat(&iface).await?;
    reconcile_in(registry_dir).await?;

    info!(resource_group_id = %resource_group_id, iface = %iface, "Resource group bridge removed");

    Ok(())
}

pub async fn list_rg_ids() -> Result<Vec<String>> {
    Ok(list_bridges(Path::new(RG_REGISTRY_DIR))
        .await?
        .into_iter()
        .map(|bridge| bridge.resource_group_id)
        .collect())
}

async fn list_bridges(registry_dir: &Path) -> Result<Vec<Bridge>> {
    let mut entries = match tokio::fs::read_dir(registry_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("failed to read resource group registry directory"),
    };

    let mut bridges = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("failed to read resource group registry entry")?
    {
        let Some(id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let iface = rg_bridge_iface_name(&id);
        if !interface_exists(&iface).await? {
            continue;
        }
        let cidr = tokio::fs::read_to_string(entry.path())
            .await
            .ok()
            .map(|content| content.trim().to_string())
            .filter(|content| !content.is_empty());
        bridges.push(Bridge {
            resource_group_id: id,
            iface,
            cidr,
        });
    }
    bridges.sort_by(|a, b| a.iface.cmp(&b.iface));

    Ok(bridges)
}

fn registry_entry_path(registry_dir: &Path, resource_group_id: &str) -> PathBuf {
    registry_dir.join(resource_group_id)
}

async fn write_registry_entry(
    registry_dir: &Path,
    resource_group_id: &str,
    cidr: Option<&str>,
) -> Result<()> {
    tokio::fs::create_dir_all(registry_dir)
        .await
        .context("failed to create resource group registry directory")?;
    tokio::fs::write(
        registry_entry_path(registry_dir, resource_group_id),
        cidr.unwrap_or_default(),
    )
    .await
    .context("failed to write resource group registry entry")?;
    Ok(())
}

async fn remove_registry_entry(registry_dir: &Path, resource_group_id: &str) {
    let _ = tokio::fs::remove_file(registry_entry_path(registry_dir, resource_group_id)).await;
}

async fn has_ipv4_address(iface: &str) -> Result<bool> {
    let output = Command::new("ip")
        .args(["-4", "-o", "addr", "show", "dev", iface])
        .output()
        .await
        .context("failed to execute ip addr show")?;
    Ok(output.status.success() && !output.stdout.is_empty())
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
        let tools = FakeTools::install(&[("ip", "exit 0"), ("nft", "exit 0")]);

        teardown_in(&registry_dir(), "rg-1").await.unwrap();

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
            let _tools = FakeTools::install(&[
                ("ip", "echo 'Cannot find device \"x\"' >&2\nexit 1"),
                ("nft", "exit 0"),
            ]);
            teardown_in(&registry_dir(), "rg-1").await.unwrap();
        }
        let _tools = FakeTools::install(&[("ip", "echo 'Operation not permitted' >&2\nexit 1")]);
        let error = teardown_in(&registry_dir(), "rg-1")
            .await
            .unwrap_err()
            .to_string();
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
        let path = registry_entry_path(Path::new(RG_REGISTRY_DIR), "rg-1");
        assert_eq!(path, Path::new(RG_REGISTRY_DIR).join("rg-1"));
    }

    const BRIDGE_STATE: &str = r#"case "$*" in
"link add"*) touch "$0.created";;
"link show"*) [ -e "$0.created" ] || exit 1;;
"-j -4 route"*) echo '[]';;
esac"#;
    const ALL_BRIDGES_PRESENT: &str = r#"case "$*" in "-4 -o addr show"*) ;; esac"#;
    const CAPTURING: &str = "cat >> \"$0.stdin\"";
    const CIDR: &str = "10.101.0.0/24";

    fn registry_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("csfx-rg-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_entry(dir: &Path, resource_group_id: &str, cidr: &str) {
        std::fs::write(dir.join(resource_group_id), cidr).unwrap();
    }

    fn position(calls: &[String], needle: &str) -> usize {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .unwrap_or_else(|| panic!("{needle} missing in {calls:?}"))
    }

    #[tokio::test]
    async fn a_bridge_left_half_configured_is_completed_by_the_next_reconcile() {
        let dir = registry_dir();
        write_entry(&dir, "rg-1", CIDR);
        let tools = FakeTools::install(&[("ip", ALL_BRIDGES_PRESENT), ("nft", CAPTURING)]);

        reconcile_in(&dir).await.unwrap();

        let iface = rg_bridge_iface_name("rg-1");
        let ip = tools.calls("ip");
        let gateway = second_host_ip(CIDR).unwrap();
        assert!(
            position(&ip, &format!("addr add {gateway}/24 dev {iface}"))
                < position(&ip, "link set dev")
        );
        let script = tools.stdin("nft");
        assert!(script.contains(&format!("iifname \"{iface}\" ct status dnat masquerade")));
        assert!(script.contains(&format!(
            "oifname \"{}\" accept",
            crate::wireguard::rg_interface_name("rg-1")
        )));
    }

    #[tokio::test]
    async fn bridges_stay_down_when_the_rules_cannot_be_applied() {
        let dir = registry_dir();
        write_entry(&dir, "rg-1", CIDR);
        let tools = FakeTools::install(&[
            ("ip", ALL_BRIDGES_PRESENT),
            ("nft", "cat > /dev/null\necho denied >&2\nexit 1"),
        ]);

        let error = reconcile_in(&dir).await.unwrap_err();

        assert!(format!("{error:#}").contains("denied"));
        assert!(tools.calls("ip").iter().all(|call| !call.contains(" up")));
    }

    #[tokio::test]
    async fn every_bridge_on_the_host_is_isolated_from_every_other() {
        let dir = registry_dir();
        write_entry(&dir, "rg-1", CIDR);
        write_entry(&dir, "rg-2", "10.102.0.0/24");
        let tools = FakeTools::install(&[("ip", ALL_BRIDGES_PRESENT), ("nft", CAPTURING)]);

        reconcile_in(&dir).await.unwrap();

        let script = tools.stdin("nft");
        let (first, second) = (rg_bridge_iface_name("rg-1"), rg_bridge_iface_name("rg-2"));
        assert!(script.contains(&format!("iifname \"{first}\" oifname \"{second}\" drop")));
        assert!(script.contains(&format!("iifname \"{second}\" oifname \"{first}\" drop")));
    }

    #[tokio::test]
    async fn legacy_entries_without_a_cidr_still_receive_rules() {
        let dir = registry_dir();
        write_entry(&dir, "rg-1", "");
        let tools = FakeTools::install(&[("ip", ALL_BRIDGES_PRESENT), ("nft", CAPTURING)]);

        reconcile_in(&dir).await.unwrap();

        let iface = rg_bridge_iface_name("rg-1");
        assert!(tools.stdin("nft").contains(&format!(
            "iifname \"{iface}\" oifname != \"{iface}\" masquerade"
        )));
        assert!(tools
            .calls("ip")
            .iter()
            .all(|call| !call.contains("addr add")));
    }

    #[tokio::test]
    async fn entries_of_vanished_bridges_cause_no_work() {
        let dir = registry_dir();
        write_entry(&dir, "rg-1", CIDR);
        let tools = FakeTools::install(&[("ip", "exit 1"), ("nft", CAPTURING)]);

        reconcile_in(&dir).await.unwrap();

        assert!(tools.calls("nft").is_empty());
    }

    #[tokio::test]
    async fn a_new_bridge_gets_its_rules_before_it_comes_up() {
        let dir = registry_dir();
        let tools = FakeTools::install(&[("ip", BRIDGE_STATE), ("nft", CAPTURING)]);
        let registry = RgDnsRegistry::new();

        let iface = ensure_bridge_in(&dir, "rg-1", Some(CIDR), &registry)
            .await
            .unwrap();

        let ip = tools.calls("ip");
        assert!(position(&ip, "link add") < position(&ip, "addr add"));
        assert!(position(&ip, "addr add") < position(&ip, "link set dev"));
        assert!(tools
            .stdin("nft")
            .contains(&format!("iifname \"{iface}\" ct status dnat masquerade")));
        assert_eq!(std::fs::read_to_string(dir.join("rg-1")).unwrap(), CIDR);
    }

    #[tokio::test]
    async fn assigned_ranges_are_recorded_once_and_corrected_when_they_change() {
        let dir = registry_dir();
        write_entry(&dir, "rg-1", "");
        let groups = vec![
            ("rg-1".to_string(), CIDR.to_string()),
            ("rg-2".to_string(), "10.102.0.0/24".to_string()),
        ];

        record_cidrs_in(&dir, &groups).await;

        assert_eq!(std::fs::read_to_string(dir.join("rg-1")).unwrap(), CIDR);
        assert_eq!(
            std::fs::read_to_string(dir.join("rg-2")).unwrap(),
            "10.102.0.0/24"
        );
        record_cidrs_in(&dir, &[("rg-1".to_string(), "10.103.0.0/24".to_string())]).await;
        assert_eq!(
            std::fs::read_to_string(dir.join("rg-1")).unwrap(),
            "10.103.0.0/24"
        );
    }
}
