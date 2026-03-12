use anyhow::{bail, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

use crate::config::Config;
use crate::verify;

pub async fn run(cfg: &Config, version: &str) -> Result<()> {
    pull(cfg, version).await?;
    verify::verify_images(cfg, version).await?;
    up(cfg, version).await?;
    health_check(cfg, version).await
}

async fn pull(cfg: &Config, version: &str) -> Result<()> {
    info!(version = %version, "pulling images");
    compose(cfg, version, &["pull"]).await
}

async fn up(cfg: &Config, version: &str) -> Result<()> {
    info!(version = %version, "restarting services");
    compose(cfg, version, &["up", "-d", "--remove-orphans"]).await
}

async fn health_check(cfg: &Config, version: &str) -> Result<()> {
    info!("waiting for health checks");
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;

    let output = Command::new("docker")
        .args(["compose", "-f", &cfg.compose_file, "ps", "--format", "json"])
        .env("GHCR_ORG", &cfg.ghcr_org)
        .env("CSF_VERSION", version)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Ok(svc) = serde_json::from_str::<serde_json::Value>(line) {
            if svc["Health"].as_str() == Some("unhealthy") {
                bail!("service {} is unhealthy after update", svc["Name"].as_str().unwrap_or("unknown"));
            }
        }
    }

    info!("all services healthy");
    Ok(())
}

async fn compose(cfg: &Config, version: &str, args: &[&str]) -> Result<()> {
    let mut cmd_args = vec!["compose", "-f", cfg.compose_file.as_str()];
    cmd_args.extend_from_slice(args);

    let status = Command::new("docker")
        .args(&cmd_args)
        .env("GHCR_ORG", &cfg.ghcr_org)
        .env("CSF_VERSION", version)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        bail!("docker compose {} failed: {}", args.join(" "), status);
    }
    Ok(())
}
