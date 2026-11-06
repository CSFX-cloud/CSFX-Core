use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::info;

const PEER_CACHE_FILE: &str = "/var/lib/csfx-agent/mgmt-peers.json";

#[derive(Serialize, Deserialize)]
pub struct Peer {
    pub public_key: String,
    pub endpoint: Option<String>,
    pub allowed_ips: String,
}

pub const MGMT_INTERFACE_NAME: &str = "wgmgmt0";

pub async fn ensure_mgmt_interface(
    private_key_b64: &str,
    listen_port: u16,
    tunnel_ip: &str,
) -> Result<()> {
    let iface = MGMT_INTERFACE_NAME;

    if !interface_exists(iface).await? {
        run_ip(&["link", "add", "dev", iface, "type", "wireguard"]).await?;
        set_private_key(iface, private_key_b64).await?;
        run_wg(&["set", iface, "listen-port", &listen_port.to_string()]).await?;
    }
    ensure_address(iface, tunnel_ip).await?;
    run_ip(&["link", "set", "up", "dev", iface]).await?;

    info!(iface = %iface, tunnel_ip = %tunnel_ip, "Management WireGuard interface ready");

    Ok(())
}

async fn ensure_address(iface: &str, tunnel_ip: &str) -> Result<()> {
    let wanted = format!("{}/32", tunnel_ip);
    let output = Command::new("ip")
        .args(["-4", "-o", "addr", "show", "dev", iface])
        .output()
        .await
        .context("failed to execute ip addr show")?;
    if has_address(&String::from_utf8_lossy(&output.stdout), &wanted) {
        return Ok(());
    }
    run_ip(&["address", "flush", "dev", iface]).await?;
    run_ip(&["address", "add", &wanted, "dev", iface]).await
}

pub fn save_peer_cache(peers: &[Peer]) -> Result<()> {
    let data = serde_json::to_string(peers).context("failed to serialize peer cache")?;
    std::fs::write(PEER_CACHE_FILE, data).context("failed to write peer cache")
}

pub fn load_peer_cache() -> Result<Vec<Peer>> {
    match std::fs::read_to_string(PEER_CACHE_FILE) {
        Ok(data) => serde_json::from_str(&data).context("failed to parse peer cache"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err).context("failed to read peer cache"),
    }
}

pub async fn apply_mgmt_peers(peers: &[Peer]) -> Result<()> {
    reconcile_peers(MGMT_INTERFACE_NAME, peers).await?;
    for peer in peers {
        if let Err(e) = set_route(MGMT_INTERFACE_NAME, &peer.allowed_ips).await {
            tracing::warn!(allowed_ips = %peer.allowed_ips, error = %e, "Failed to set management route");
        }
    }
    Ok(())
}

pub fn rg_interface_name(resource_group_id: &str) -> String {
    const FNV_OFFSET_BASIS: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in resource_group_id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("wgrg{:08x}", hash)
}

pub async fn ensure_interface(
    resource_group_id: &str,
    private_key_b64: &str,
    listen_port: u16,
) -> Result<String> {
    let iface = rg_interface_name(resource_group_id);

    if interface_exists(&iface).await? {
        return Ok(iface);
    }

    run_ip(&["link", "add", "dev", &iface, "type", "wireguard"]).await?;
    set_private_key(&iface, private_key_b64).await?;
    run_wg(&["set", &iface, "listen-port", &listen_port.to_string()]).await?;
    run_ip(&["link", "set", "up", "dev", &iface]).await?;

    info!(resource_group_id = %resource_group_id, iface = %iface, "WireGuard interface ready");

    Ok(iface)
}

pub async fn set_route(iface: &str, cidr: &str) -> Result<()> {
    let output = Command::new("ip")
        .args(["route", "add", cidr, "dev", iface])
        .output()
        .await
        .context("failed to execute ip route add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("File exists") {
            return Ok(());
        }
        anyhow::bail!(
            "ip route add failed cidr={} iface={} stderr={}",
            cidr,
            iface,
            stderr
        );
    }

    Ok(())
}

pub async fn sync_host_routes(
    iface: &str,
    desired: &std::collections::HashSet<String>,
) -> Result<Vec<String>> {
    let current = list_host_routes(iface).await?;

    for stale in current.difference(desired) {
        run_ip(&["route", "del", &format!("{}/32", stale), "dev", iface]).await?;
    }

    let added: Vec<String> = desired.difference(&current).cloned().collect();
    for address in &added {
        run_ip(&["route", "replace", &format!("{}/32", address), "dev", iface]).await?;
    }

    Ok(added)
}

async fn list_host_routes(iface: &str) -> Result<std::collections::HashSet<String>> {
    let output = Command::new("ip")
        .args(["-4", "route", "show", "dev", iface])
        .output()
        .await
        .context("failed to execute ip route show")?;

    if !output.status.success() {
        anyhow::bail!(
            "ip route show failed iface={} stderr={}",
            iface,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(parse_host_routes(&String::from_utf8_lossy(&output.stdout)))
}

fn has_address(listing: &str, wanted: &str) -> bool {
    listing.split_whitespace().any(|field| field == wanted)
}

fn parse_host_routes(listing: &str) -> std::collections::HashSet<String> {
    listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|destination| destination.strip_suffix("/32").or(Some(destination)))
        .filter(|destination| destination.parse::<std::net::Ipv4Addr>().is_ok())
        .map(str::to_string)
        .collect()
}

fn parse_peer_keys(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn peer_arguments(iface: &str, peer: &Peer) -> Vec<String> {
    let mut args = vec![
        "set".to_string(),
        iface.to_string(),
        "peer".to_string(),
        peer.public_key.clone(),
        "allowed-ips".to_string(),
        peer.allowed_ips.clone(),
    ];
    if let Some(endpoint) = &peer.endpoint {
        args.extend([
            "endpoint".to_string(),
            endpoint.clone(),
            "persistent-keepalive".to_string(),
            "25".to_string(),
        ]);
    }
    args
}

pub async fn remove_interface(iface: &str) -> Result<()> {
    let output = Command::new("ip")
        .args(["link", "delete", "dev", iface])
        .output()
        .await
        .context("failed to execute ip link delete")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Cannot find device") {
            return Ok(());
        }
        anyhow::bail!("ip link delete failed iface={} stderr={}", iface, stderr);
    }

    info!(iface = %iface, "WireGuard interface removed");

    Ok(())
}

pub async fn set_peers(iface: &str, peers: &[Peer]) -> Result<()> {
    for peer in peers {
        let args = peer_arguments(iface, peer);
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_wg(&arg_refs).await?;
    }

    Ok(())
}

pub async fn reconcile_peers(iface: &str, peers: &[Peer]) -> Result<()> {
    let current = list_peer_public_keys(iface).await?;
    let desired: std::collections::HashSet<&str> =
        peers.iter().map(|p| p.public_key.as_str()).collect();

    for stale_key in current.iter().filter(|k| !desired.contains(k.as_str())) {
        run_wg(&["set", iface, "peer", stale_key, "remove"]).await?;
        info!(iface = %iface, public_key = %stale_key, "Removed stale WireGuard peer");
    }

    set_peers(iface, peers).await
}

async fn list_peer_public_keys(iface: &str) -> Result<Vec<String>> {
    let output = Command::new("wg")
        .args(["show", iface, "peers"])
        .output()
        .await
        .context("failed to execute wg show peers")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("wg show peers failed iface={} stderr={}", iface, stderr);
    }

    Ok(parse_peer_keys(&String::from_utf8_lossy(&output.stdout)))
}

async fn interface_exists(iface: &str) -> Result<bool> {
    let output = Command::new("ip")
        .args(["link", "show", iface])
        .output()
        .await
        .context("failed to execute ip link show")?;

    Ok(output.status.success())
}

async fn set_private_key(iface: &str, private_key_b64: &str) -> Result<()> {
    let mut child = tokio::process::Command::new("wg")
        .args(["set", iface, "private-key", "/dev/stdin"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("failed to spawn wg set private-key")?;

    let mut stdin = child.stdin.take().context("failed to open wg stdin")?;
    stdin
        .write_all(private_key_b64.as_bytes())
        .await
        .context("failed to write private key to wg stdin")?;
    drop(stdin);

    let status = child
        .wait()
        .await
        .context("failed to wait on wg set private-key")?;

    if !status.success() {
        anyhow::bail!("wg set private-key failed iface={}", iface);
    }

    Ok(())
}

async fn run_ip(args: &[&str]) -> Result<()> {
    let output = Command::new("ip")
        .args(args)
        .output()
        .await
        .context("failed to execute ip")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ip command failed args={:?} stderr={}", args, stderr);
    }

    Ok(())
}

async fn run_wg(args: &[&str]) -> Result<()> {
    let output = Command::new("wg")
        .args(args)
        .output()
        .await
        .context("failed to execute wg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("wg command failed args={:?} stderr={}", args, stderr);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_name_is_stable_and_prefixed() {
        let name = rg_interface_name("rg-1");
        assert_eq!(name, rg_interface_name("rg-1"));
        assert_ne!(name, rg_interface_name("rg-2"));
        assert!(name.starts_with("wgrg"));
        assert_eq!(name.len(), 12);
        assert!(name.len() <= 15);
    }

    #[test]
    fn peer_round_trips_through_json() {
        let peers = vec![Peer {
            public_key: "key".to_string(),
            endpoint: None,
            allowed_ips: "10.0.0.1/32".to_string(),
        }];
        let raw = serde_json::to_string(&peers).unwrap();
        let parsed: Vec<Peer> = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed[0].public_key, "key");
        assert!(parsed[0].endpoint.is_none());
        assert_eq!(parsed[0].allowed_ips, "10.0.0.1/32");
    }

    #[test]
    fn address_is_matched_as_a_whole_field() {
        let listing = "5: wgmgmt0    inet 10.100.0.7/32 scope global wgmgmt0";
        assert!(has_address(listing, "10.100.0.7/32"));
        assert!(!has_address(listing, "10.100.0.7/3"));
        assert!(!has_address(listing, "10.100.0.8/32"));
        assert!(!has_address("", "10.100.0.7/32"));
    }

    #[test]
    fn host_routes_keep_only_ipv4_destinations() {
        let listing = "10.0.0.5 scope link\n10.0.0.6/32 scope link\n10.0.1.0/24 proto kernel\ndefault via 10.0.0.1\n\n";
        let routes = parse_host_routes(listing);
        assert_eq!(routes.len(), 2);
        assert!(routes.contains("10.0.0.5"));
        assert!(routes.contains("10.0.0.6"));
    }

    #[test]
    fn peer_keys_skip_blank_lines() {
        assert_eq!(
            parse_peer_keys("key-a\n  key-b  \n\n"),
            vec!["key-a".to_string(), "key-b".to_string()]
        );
        assert!(parse_peer_keys("\n").is_empty());
    }

    #[test]
    fn peers_with_endpoints_get_a_keepalive() {
        let mut peer = Peer {
            public_key: "key".to_string(),
            endpoint: None,
            allowed_ips: "10.0.0.1/32".to_string(),
        };
        assert_eq!(
            peer_arguments("wg0", &peer),
            ["set", "wg0", "peer", "key", "allowed-ips", "10.0.0.1/32"]
        );
        peer.endpoint = Some("192.0.2.1:51820".to_string());
        let args = peer_arguments("wg0", &peer);
        assert_eq!(args.len(), 10);
        assert_eq!(
            args[6..],
            ["endpoint", "192.0.2.1:51820", "persistent-keepalive", "25"]
        );
    }

    use crate::testing::FakeTools;
    use std::collections::HashSet;

    const IP_MISSING_INTERFACE: &str = r#"case "$*" in "link show"*) exit 1;; esac"#;
    const WG_READS_KEY: &str = r#"case "$*" in *private-key*) cat > /dev/null;; esac"#;

    fn peer(key: &str, endpoint: Option<&str>) -> Peer {
        Peer {
            public_key: key.to_string(),
            endpoint: endpoint.map(str::to_string),
            allowed_ips: format!("10.0.0.{}/32", key.len()),
        }
    }

    #[tokio::test]
    async fn management_interface_is_created_configured_and_addressed() {
        let tools = FakeTools::install(&[("ip", IP_MISSING_INTERFACE), ("wg", WG_READS_KEY)]);

        ensure_mgmt_interface("c2VjcmV0", 51820, "10.100.0.7")
            .await
            .unwrap();

        let ip = tools.calls("ip");
        assert!(ip.contains(&"link add dev wgmgmt0 type wireguard".to_string()));
        assert!(ip.contains(&"address flush dev wgmgmt0".to_string()));
        assert!(ip.contains(&"address add 10.100.0.7/32 dev wgmgmt0".to_string()));
        assert_eq!(ip.last().unwrap(), "link set up dev wgmgmt0");
        let wg = tools.calls("wg");
        assert!(wg.contains(&"set wgmgmt0 private-key /dev/stdin".to_string()));
        assert!(wg.contains(&"set wgmgmt0 listen-port 51820".to_string()));
    }

    #[tokio::test]
    async fn existing_management_interface_keeps_a_matching_address() {
        let tools = FakeTools::install(&[(
            "ip",
            r#"case "$*" in "-4 -o addr show"*) echo '5: wgmgmt0 inet 10.100.0.7/32 scope global';; esac"#,
        )]);

        ensure_mgmt_interface("c2VjcmV0", 51820, "10.100.0.7")
            .await
            .unwrap();

        let ip = tools.calls("ip");
        assert!(!ip.iter().any(|call| call.contains("link add")));
        assert!(!ip.iter().any(|call| call.contains("address flush")));
        assert!(tools.calls("wg").is_empty());
    }

    #[tokio::test]
    async fn group_interfaces_are_created_once() {
        let tools = FakeTools::install(&[("ip", IP_MISSING_INTERFACE), ("wg", WG_READS_KEY)]);

        let name = ensure_interface("rg-1", "c2VjcmV0", 51900).await.unwrap();

        assert_eq!(name, rg_interface_name("rg-1"));
        assert!(tools
            .calls("ip")
            .contains(&format!("link add dev {name} type wireguard")));
        drop(tools);

        let tools = FakeTools::install(&[("ip", SUCCEEDING), ("wg", SUCCEEDING)]);
        ensure_interface("rg-1", "c2VjcmV0", 51900).await.unwrap();
        assert_eq!(tools.calls("ip").len(), 1);
        assert!(tools.calls("wg").is_empty());
    }

    const SUCCEEDING: &str = "exit 0";

    #[tokio::test]
    async fn peers_are_configured_with_keepalive_only_when_reachable() {
        let tools = FakeTools::install(&[("wg", SUCCEEDING)]);

        set_peers(
            "wg0",
            &[peer("a", Some("192.0.2.1:51820")), peer("bb", None)],
        )
        .await
        .unwrap();

        let calls = tools.calls("wg");
        assert_eq!(calls.len(), 2);
        assert!(calls[0].ends_with("endpoint 192.0.2.1:51820 persistent-keepalive 25"));
        assert!(!calls[1].contains("endpoint"));
    }

    #[tokio::test]
    async fn stale_peers_are_removed_during_reconciliation() {
        let tools = FakeTools::install(&[(
            "wg",
            r#"case "$*" in "show wg0 peers") printf '%s\n' keep old;; esac"#,
        )]);

        reconcile_peers("wg0", &[peer("keep", None)]).await.unwrap();

        let calls = tools.calls("wg");
        assert!(calls.contains(&"set wg0 peer old remove".to_string()));
        assert!(!calls.contains(&"set wg0 peer keep remove".to_string()));
        assert!(calls
            .iter()
            .any(|call| call.starts_with("set wg0 peer keep allowed-ips")));
    }

    #[tokio::test]
    async fn reconciliation_fails_when_the_peer_listing_fails() {
        let _tools = FakeTools::install(&[("wg", "echo nope >&2\nexit 1")]);

        let error = reconcile_peers("wg0", &[]).await.unwrap_err().to_string();

        assert!(error.contains("wg show peers failed"));
    }

    #[tokio::test]
    async fn management_peers_get_routes_and_route_errors_are_tolerated() {
        let tools = FakeTools::install(&[
            ("wg", "case \"$*\" in \"show\"*) ;; esac"),
            ("ip", "echo 'RTNETLINK: boom' >&2\nexit 2"),
        ]);

        apply_mgmt_peers(&[peer("a", None), peer("bb", None)])
            .await
            .unwrap();

        let routes = tools.calls("ip");
        assert_eq!(routes.len(), 2);
        assert!(routes[0].starts_with("route add 10.0.0.1/32 dev wgmgmt0"));
    }

    #[tokio::test]
    async fn existing_routes_are_not_an_error() {
        {
            let _tools =
                FakeTools::install(&[("ip", "echo 'RTNETLINK answers: File exists' >&2\nexit 2")]);
            set_route("wg0", "10.0.0.0/24").await.unwrap();
        }
        let _tools = FakeTools::install(&[("ip", "echo 'bad' >&2\nexit 2")]);
        let error = set_route("wg0", "10.0.0.0/24")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("ip route add failed"));
    }

    #[tokio::test]
    async fn host_routes_are_synchronised_to_the_desired_set() {
        let tools = FakeTools::install(&[(
            "ip",
            r#"case "$*" in "-4 route show"*) printf '%s\n' '10.0.0.5 scope link' '10.0.0.6/32 scope link' 'default via 10.0.0.1';; esac"#,
        )]);
        let desired: HashSet<String> = ["10.0.0.6", "10.0.0.7"].map(String::from).into();

        let added = sync_host_routes("wg0", &desired).await.unwrap();

        assert_eq!(added, vec!["10.0.0.7".to_string()]);
        let calls = tools.calls("ip");
        assert!(calls.contains(&"route del 10.0.0.5/32 dev wg0".to_string()));
        assert!(calls.contains(&"route replace 10.0.0.7/32 dev wg0".to_string()));
        assert!(!calls
            .iter()
            .any(|call| call.contains("10.0.0.6/32 dev wg0") && call.starts_with("route replace")));
    }

    #[tokio::test]
    async fn host_route_listing_failures_are_reported() {
        let _tools = FakeTools::install(&[("ip", "echo bad >&2\nexit 1")]);

        let error = sync_host_routes("wg0", &HashSet::new())
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("ip route show failed"));
    }

    #[tokio::test]
    async fn removing_a_missing_interface_succeeds() {
        {
            let tools = FakeTools::install(&[("ip", SUCCEEDING)]);
            remove_interface("wg0").await.unwrap();
            assert_eq!(tools.calls("ip"), vec!["link delete dev wg0".to_string()]);
        }
        {
            let _tools =
                FakeTools::install(&[("ip", "echo 'Cannot find device \"wg0\"' >&2\nexit 1")]);
            remove_interface("wg0").await.unwrap();
        }
        let _tools = FakeTools::install(&[("ip", "echo denied >&2\nexit 1")]);
        assert!(remove_interface("wg0").await.is_err());
    }

    #[tokio::test]
    async fn failing_commands_surface_their_arguments() {
        let _tools = FakeTools::install(&[("ip", "exit 1"), ("wg", "exit 1")]);

        assert!(run_ip(&["link", "show"])
            .await
            .unwrap_err()
            .to_string()
            .contains("ip command failed"));
        assert!(run_wg(&["show"])
            .await
            .unwrap_err()
            .to_string()
            .contains("wg command failed"));
        assert!(set_private_key("wg0", "key").await.is_err());
    }
}
