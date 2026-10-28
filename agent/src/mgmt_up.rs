use anyhow::{Context, Result};
use tracing::info;

use crate::{config, wg_identity, wireguard, MGMT_WG_PORT};

pub async fn run() -> Result<()> {
    let tunnel_ip = config::load_config()?
        .wg_tunnel_ip
        .context("daemon config has no management tunnel ip")?;
    let identity = wg_identity::load_or_generate()?;
    wireguard::ensure_mgmt_interface(&identity.private_key_b64, MGMT_WG_PORT, &tunnel_ip).await?;
    let peers = wireguard::load_peer_cache()?;
    wireguard::apply_mgmt_peers(&peers).await?;
    info!(tunnel_ip = %tunnel_ip, peers = peers.len(), "management interface restored from cache");
    Ok(())
}
