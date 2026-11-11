mod apply;
mod channel;
mod config;
mod etcd;
mod health;
mod manifest;
mod nix;
mod rollout;
mod state;
mod version;

use anyhow::{bail, Result};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info};

use apply::Outcome;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install ring crypto provider"))?;

    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::from_env()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => run_daemon(&cfg).await,
        Some("apply") => run_apply(&cfg, &args[1..]).await,
        Some(other) => bail!("unknown command {other}, expected apply"),
    }
}

async fn run_apply(cfg: &config::Config, args: &[String]) -> Result<()> {
    let force = args.iter().any(|arg| arg == "--force");
    let Some(manifest_path) = args.iter().find(|arg| !arg.starts_with("--")) else {
        bail!("usage: csfx-updater apply <manifest> [--force]");
    };
    match apply::apply(cfg, &PathBuf::from(manifest_path), force).await? {
        Outcome::Applied => info!("update applied"),
        Outcome::AlreadyCurrent => info!("system already on this release"),
        Outcome::RolledBack => bail!("update failed health check and was rolled back"),
    }
    Ok(())
}

async fn run_daemon(cfg: &config::Config) -> Result<()> {
    info!(poll_interval_secs = cfg.poll_interval_secs, release_base_url = %cfg.release_base_url, "csfx-updater started");
    let interval = Duration::from_secs(cfg.poll_interval_secs);
    loop {
        if let Err(error) = rollout::run_cycle(cfg).await {
            error!(%error, "update cycle failed");
        }
        tokio::time::sleep(interval).await;
    }
}
