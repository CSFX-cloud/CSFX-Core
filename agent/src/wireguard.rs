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
    let current = String::from_utf8_lossy(&output.stdout);
    if current.split_whitespace().any(|field| field == wanted) {
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

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|destination| destination.strip_suffix("/32").or(Some(destination)))
        .filter(|destination| destination.parse::<std::net::Ipv4Addr>().is_ok())
        .map(str::to_string)
        .collect())
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
        let mut args = vec![
            "set".to_string(),
            iface.to_string(),
            "peer".to_string(),
            peer.public_key.clone(),
            "allowed-ips".to_string(),
            peer.allowed_ips.clone(),
        ];

        if let Some(endpoint) = &peer.endpoint {
            args.push("endpoint".to_string());
            args.push(endpoint.clone());
            args.push("persistent-keepalive".to_string());
            args.push("25".to_string());
        }

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

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
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
