mod agent_stream;
mod arp;
mod backoff;
mod bootstrap;
mod cidr;
mod client;
mod cmd;
mod config;
mod disks;
mod endpoint;
mod fence;
mod firecracker;
mod heartbeat;
mod isolation;
mod join;
mod journal;
mod local_state;
mod master_join;
mod mgmt_up;
mod nftables;
mod pki;
mod qemu;
mod rbd;
mod reconcile;
mod rg_dhcp;
mod rg_dns;
mod rg_dns_process;
mod rg_ipam;
mod rg_mesh;
mod rg_network;
mod runtime;
mod server;
mod spec;
mod ssh_control;
mod ssh_keys;
mod start_tracker;
mod state;
mod system;
#[cfg(test)]
mod testing;
mod update_watch;
mod vip_sync;
mod vm_host;
mod wg_identity;
mod wireguard;

use anyhow::{Context, Result};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let mut args = std::env::args().skip(1);
    let command = args.next();
    if command.as_deref() == Some("--mgmt-up") {
        return mgmt_up::run().await;
    }
    if command.as_deref() == Some("--authorized-keys") {
        return authorized_keys(args.next()).await;
    }

    let join_ticket = bootstrap::join_ticket()?;
    let gateway_url = bootstrap::gateway_url(join_ticket.as_ref())?;
    match command.as_deref() {
        Some("--join-master") => master_join::run(&gateway_url).await,
        Some("--sync-vip") => vip_sync::run(&gateway_url).await,
        _ => bootstrap::run(gateway_url, join_ticket).await,
    }
}

async fn authorized_keys(username: Option<String>) -> Result<()> {
    let Some(username) = username else {
        info!("No username given, nothing to authorize");
        return Ok(());
    };
    let config = config::load_config().context("Failed to load daemon config")?;
    let gateway_url = bootstrap::effective_gateway_url(config.gateway_url);
    ssh_keys::run_authorized_keys_command(&gateway_url, config.agent_id, &username).await;
    Ok(())
}
