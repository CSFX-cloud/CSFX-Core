use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::config::MGMT_WG_PORT;
use crate::reconcile::{MountedVolumes, Reconciler, Runtimes};
use crate::rg_dns::RgDnsRegistry;
use crate::start_tracker::StartTracker;
use crate::state::WorkloadTable;
use crate::{
    agent_stream, client, config, endpoint, firecracker, heartbeat, join, journal, local_state,
    pki, qemu, server, system, wg_identity, wireguard,
};

const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 30;

pub fn effective_gateway_url(gateway_url: String) -> String {
    config::load_gateway_override()
        .filter(|_| !config::is_loopback_url(&gateway_url))
        .unwrap_or(gateway_url)
}

pub fn join_ticket() -> Result<Option<join::JoinTicket>> {
    let ticket = std::env::var("CSFX_JOIN")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| join::JoinTicket::parse(&value))
        .transpose()
        .context("Invalid CSFX_JOIN")?;
    if let Some(ticket) = &ticket {
        discard_foreign_registration(ticket)?;
    }
    Ok(ticket)
}

pub fn gateway_url(join_ticket: Option<&join::JoinTicket>) -> Result<String> {
    let gateway_url = match join_ticket {
        Some(ticket) => ticket.gateway_url.clone(),
        None => std::env::var("CSFX_GATEWAY_URL")
            .context("CSFX_GATEWAY_URL environment variable is required")?,
    };
    Ok(effective_gateway_url(gateway_url))
}

pub async fn run(gateway_url: String, join_ticket: Option<join::JoinTicket>) -> Result<()> {
    info!(version = env!("CARGO_PKG_VERSION"), "csfx-agent starting");

    if !system::is_kvm_capable() {
        error!("no /dev/kvm found, this host cannot schedule any workloads");
    }

    let interval_secs: u64 = std::env::var("CSFX_HEARTBEAT_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SECS);
    let wg_identity =
        wg_identity::load_or_generate().context("Failed to initialize WireGuard identity")?;
    let api_client = build_client(&gateway_url, join_ticket.as_ref(), &wg_identity).await?;
    let agent_pki = pki::AgentPki::load_or_generate().context("Failed to initialize PKI")?;

    let (agent_id, api_key) = if config::is_registered() {
        info!("Existing registration found, loading credentials");
        load_registration()?
    } else {
        info!("No registration found, starting registration");
        register(
            &api_client,
            &gateway_url,
            interval_secs,
            &agent_pki,
            join_ticket.as_ref(),
        )
        .await?
    };

    let api_client = Arc::new(with_certificate(api_client));
    info!(agent_id = %agent_id, "Agent registered, starting heartbeat loop");
    bring_up_mgmt_interface(&wg_identity).await;

    let rg_dns = Arc::new(RgDnsRegistry::new());
    let local_state = Arc::new(local_state::LocalState::open(local_state::STATE_FILE));
    let runtimes = build_runtimes(&wg_identity, &local_state, &rg_dns, &gateway_url, &api_key);
    let workloads = WorkloadTable::default();

    spawn_inbound_server(&runtimes, &workloads, &api_client, agent_id, &api_key);
    journal::spawn(api_client.clone(), api_key.clone());
    let assignment_signal = agent_stream::spawn(gateway_url, api_key.clone(), agent_id);

    let reconciler = Reconciler {
        client: api_client,
        api_key: Arc::from(api_key),
        agent_id,
        runtimes,
        workloads,
        volumes: MountedVolumes::default(),
        starts: StartTracker::from_env(),
        rg_dns,
        local_state,
    };
    heartbeat::run(reconciler, interval_secs, assignment_signal).await;
    Ok(())
}

async fn build_client(
    gateway_url: &str,
    join_ticket: Option<&join::JoinTicket>,
    wg_identity: &wg_identity::WgIdentity,
) -> Result<client::ApiClient> {
    let wg_endpoint = endpoint::resolve(MGMT_WG_PORT).await;
    let wg_tunnel_ip = if config::is_registered() {
        config::load_config().ok().and_then(|cfg| cfg.wg_tunnel_ip)
    } else {
        std::env::var("CSFX_WG_TUNNEL_IP").ok()
    };
    let ca_pem = resolve_trust_anchor(gateway_url, join_ticket).await?;
    client::ApiClient::new(
        gateway_url.to_string(),
        wg_identity.public_key_b64.clone(),
        wg_endpoint,
        wg_tunnel_ip,
        ca_pem.as_deref(),
    )
    .context("Failed to initialize API client")
}

fn load_registration() -> Result<(uuid::Uuid, String)> {
    let cfg = config::load_config().context("Failed to load daemon config")?;
    let creds = config::load_credentials().context("Failed to load credentials")?;
    Ok((cfg.agent_id, creds.api_key))
}

fn with_certificate(api_client: client::ApiClient) -> client::ApiClient {
    if !pki::AgentPki::has_certificate() {
        return api_client;
    }
    match pki::AgentPki::load_cert_pem() {
        Ok(cert_pem) => {
            info!("mTLS: client certificate loaded");
            api_client.with_certificate(cert_pem)
        }
        Err(e) => {
            warn!(error = %e, "mTLS: failed to load certificate, continuing without");
            api_client
        }
    }
}

async fn bring_up_mgmt_interface(wg_identity: &wg_identity::WgIdentity) {
    let Some(tunnel_ip) = config::load_config().ok().and_then(|cfg| cfg.wg_tunnel_ip) else {
        warn!("No management tunnel IP available, VPN peering disabled for this agent");
        return;
    };
    if let Err(e) =
        wireguard::ensure_mgmt_interface(&wg_identity.private_key_b64, MGMT_WG_PORT, &tunnel_ip)
            .await
    {
        warn!(error = %e, "Failed to bring up management WireGuard interface");
    }
}

fn build_runtimes(
    wg_identity: &wg_identity::WgIdentity,
    local_state: &Arc<local_state::LocalState>,
    rg_dns: &Arc<RgDnsRegistry>,
    gateway_url: &str,
    api_key: &str,
) -> Runtimes {
    Runtimes {
        firecracker: Arc::new(firecracker::runtime::FirecrackerRuntime::new(
            wg_identity.private_key_b64.clone(),
            Arc::clone(local_state),
            Arc::clone(rg_dns),
            firecracker::site_registry::SiteRegistry::from_gateway(gateway_url, api_key),
        )),
        qemu: Arc::new(qemu::runtime::QemuRuntime::new(
            wg_identity.private_key_b64.clone(),
            Arc::clone(local_state),
            Arc::clone(rg_dns),
        )),
    }
}

fn spawn_inbound_server(
    runtimes: &Runtimes,
    workloads: &WorkloadTable,
    api_client: &Arc<client::ApiClient>,
    agent_id: uuid::Uuid,
    api_key: &str,
) {
    let Some(port) = std::env::var("CSFX_AGENT_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
    else {
        info!("CSFX_AGENT_PORT not set, agent inbound server disabled");
        return;
    };
    let ca_key = match server::load_ca_key() {
        Ok(key) => Arc::new(key),
        Err(e) => {
            error!(error = %e, "agent inbound server disabled, trusted CA unavailable");
            return;
        }
    };
    let server_state = server::ServerState {
        firecracker: runtimes.firecracker.clone(),
        qemu: runtimes.qemu.clone(),
        workloads: workloads.clone(),
        agent_id,
        api_client: api_client.clone(),
        api_key: api_key.to_string(),
        ca_key,
    };
    tokio::spawn(async move {
        if let Err(e) = server::run(server_state, port).await {
            error!(error = %e, "agent inbound server stopped");
        }
    });
}

fn discard_foreign_registration(ticket: &join::JoinTicket) -> Result<()> {
    let ca_pem = pki::AgentPki::load_ca_pem().ok();
    let saved_gateway = config::load_config().ok().map(|saved| saved.gateway_url);
    if !ticket.is_foreign_to(ca_pem.as_deref(), saved_gateway.as_deref()) {
        return Ok(());
    }
    warn!(
        gateway_url = %ticket.gateway_url,
        "Local registration belongs to another cluster, discarding it"
    );
    config::clear_registration()?;
    pki::AgentPki::clear_certificate()
}

async fn resolve_trust_anchor(
    gateway_url: &str,
    join_ticket: Option<&join::JoinTicket>,
) -> Result<Option<String>> {
    if let Ok(ca_pem) = pki::AgentPki::load_ca_pem() {
        return Ok(Some(ca_pem));
    }
    let Some(ticket) = join_ticket else {
        return Ok(None);
    };
    let ca_pem = client::fetch_cluster_ca(gateway_url).await?;
    ticket.verify_ca(&ca_pem)?;
    info!("Cluster CA fingerprint verified, pinning gateway certificate to cluster CA");
    Ok(Some(ca_pem))
}

fn resolve_registration_token(join_ticket: Option<&join::JoinTicket>) -> Result<String> {
    if let Some(ticket) = join_ticket {
        return Ok(ticket.token.clone());
    }
    if let Some(token) = std::env::var("CSFX_REGISTRATION_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
    {
        return Ok(token);
    }
    anyhow::bail!("Neither CSFX_JOIN nor CSFX_REGISTRATION_TOKEN is set")
}

async fn register(
    client: &client::ApiClient,
    gateway_url: &str,
    heartbeat_interval_secs: u64,
    agent_pki: &pki::AgentPki,
    join_ticket: Option<&join::JoinTicket>,
) -> Result<(uuid::Uuid, String)> {
    let token = resolve_registration_token(join_ticket)?;
    let info = system::collect_info();

    info!(
        hostname = %info.hostname,
        os_type = %info.os_type,
        architecture = %info.architecture,
        "Registering with registry"
    );

    let request = client::RegisterRequest {
        registration_token: &token,
        name: &info.hostname,
        hostname: &info.hostname,
        os_type: &info.os_type,
        os_version: &info.os_version,
        architecture: &info.architecture,
        agent_version: env!("CARGO_PKG_VERSION"),
        machine_id: info.machine_id.as_deref(),
        csr_pem: agent_pki.csr_pem(),
    };
    let resp = client
        .register(&request)
        .await
        .context("Registration request failed")?;

    store_certificate(&resp)?;
    adopt_tunnel_ip(client, &resp).await;

    let cfg = config::DaemonConfig {
        gateway_url: gateway_url.to_string(),
        agent_id: resp.agent_id,
        heartbeat_interval_secs,
        wg_tunnel_ip: resp.wg_tunnel_ip.clone(),
    };
    config::save_config(&cfg).context("Failed to save daemon config")?;
    config::save_credentials(&resp.api_key).context("Failed to save credentials")?;

    info!(agent_id = %resp.agent_id, "Registration successful");
    Ok((resp.agent_id, resp.api_key))
}

fn store_certificate(resp: &client::RegisterResponse) -> Result<()> {
    let (Some(cert_pem), Some(ca_pem)) = (&resp.certificate_pem, &resp.ca_cert_pem) else {
        warn!("Registry did not issue a certificate during registration");
        return Ok(());
    };
    pki::AgentPki::save_certificate(cert_pem, ca_pem).context("Failed to save certificate")?;
    info!("PKI: certificate received and stored");
    Ok(())
}

async fn adopt_tunnel_ip(client: &client::ApiClient, resp: &client::RegisterResponse) {
    let Some(ip) = &resp.wg_tunnel_ip else {
        warn!("Registry did not assign a management tunnel IP");
        return;
    };
    client.set_wg_tunnel_ip(ip.clone()).await;
    info!(wg_tunnel_ip = %ip, "Management tunnel IP assigned by registry");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_gateways_ignore_the_override() {
        let local = "http://127.0.0.1:8000".to_string();
        assert_eq!(effective_gateway_url(local.clone()), local);
    }
}
