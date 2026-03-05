mod client;
mod config;
mod pki;
mod system;

use anyhow::{Context, Result};
use std::time::Duration;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), "csf-agent starting");

    let gateway_url = std::env::var("CSF_GATEWAY_URL")
        .context("CSF_GATEWAY_URL environment variable is required")?;

    let heartbeat_interval_secs: u64 = std::env::var("CSF_HEARTBEAT_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    let api_client = client::ApiClient::new(gateway_url.clone())
        .context("Failed to initialize API client")?;

    let agent_pki = pki::AgentPki::load_or_generate()
        .context("Failed to initialize PKI")?;

    let (agent_id, api_key) = if config::is_registered() {
        info!("Existing registration found, loading credentials");
        let cfg = config::load_config().context("Failed to load daemon config")?;
        let creds = config::load_credentials().context("Failed to load credentials")?;
        (cfg.agent_id, creds.api_key)
    } else {
        info!("No registration found, starting registration");
        perform_registration(&api_client, &gateway_url, heartbeat_interval_secs, &agent_pki)
            .await?
    };

    let api_client = if pki::AgentPki::has_certificate() {
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
    } else {
        api_client
    };

    info!(agent_id = %agent_id, "Agent registered, starting heartbeat loop");

    run_heartbeat_loop(&api_client, agent_id, &api_key, heartbeat_interval_secs).await;

    Ok(())
}

async fn perform_registration(
    client: &client::ApiClient,
    gateway_url: &str,
    heartbeat_interval_secs: u64,
    agent_pki: &pki::AgentPki,
) -> Result<(uuid::Uuid, String)> {
    let token = std::env::var("CSF_REGISTRATION_TOKEN")
        .context("CSF_REGISTRATION_TOKEN is required for first-time registration")?;

    let info = system::collect_info();

    info!(
        hostname = %info.hostname,
        os_type = %info.os_type,
        architecture = %info.architecture,
        "Registering with registry"
    );

    let resp = client
        .register(
            &token,
            &info.hostname,
            &info.hostname,
            &info.os_type,
            &info.os_version,
            &info.architecture,
            agent_pki.csr_pem(),
        )
        .await
        .context("Registration request failed")?;

    if let (Some(cert_pem), Some(ca_pem)) = (&resp.certificate_pem, &resp.ca_cert_pem) {
        pki::AgentPki::save_certificate(cert_pem, ca_pem)
            .context("Failed to save certificate")?;
        info!("PKI: certificate received and stored");
    } else {
        warn!("Registry did not issue a certificate during registration");
    }

    let cfg = config::DaemonConfig {
        gateway_url: gateway_url.to_string(),
        agent_id: resp.agent_id,
        heartbeat_interval_secs,
    };

    config::save_config(&cfg).context("Failed to save daemon config")?;
    config::save_credentials(&resp.api_key).context("Failed to save credentials")?;

    info!(agent_id = %resp.agent_id, "Registration successful");

    Ok((resp.agent_id, resp.api_key))
}

async fn run_heartbeat_loop(
    client: &client::ApiClient,
    agent_id: uuid::Uuid,
    api_key: &str,
    interval_secs: u64,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    let mut failure_count: u32 = 0;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                match client.heartbeat(agent_id, api_key).await {
                    Ok(_) => {
                        if failure_count > 0 {
                            info!(agent_id = %agent_id, "Heartbeat recovered after {} failures", failure_count);
                            failure_count = 0;
                        }
                    }
                    Err(e) => {
                        failure_count += 1;
                        warn!(
                            agent_id = %agent_id,
                            failures = failure_count,
                            error = %e,
                            "Heartbeat failed"
                        );
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    if failure_count > 0 {
        error!(failures = failure_count, "Agent shutting down with unresolved heartbeat failures");
    }
}
