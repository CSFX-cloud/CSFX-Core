use anyhow::{bail, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

use crate::config::Config;
use crate::etcd;
use crate::secret::decrypt_secret;
use crate::verify;

pub async fn run(cfg: &Config, version: &str, etcd: &mut etcd::Client) -> Result<()> {
    let (docker_config_dir, ghcr_auth) = setup_docker_auth(cfg, etcd).await?;
    pull(cfg, version, docker_config_dir.as_deref()).await?;
    verify::verify_images(cfg, version, ghcr_auth.as_deref()).await?;
    up(cfg, version, docker_config_dir.as_deref()).await?;
    health_check(cfg, version).await
}

async fn setup_docker_auth(cfg: &Config, etcd: &mut etcd::Client) -> Result<(Option<String>, Option<String>)> {
    let encrypted = match etcd.get(etcd::GHCR_TOKEN_KEY).await? {
        Some(v) => v,
        None => return Ok((None, None)),
    };

    let payload = decrypt_secret(&encrypted, &cfg.secret_encryption_key)?;
    let (username, token) = payload
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid ghcr token payload"))?;

    let dir = tempfile::tempdir()?;
    let config_path = dir.path().join("config.json");

    let auth_raw = format!("{}:{}", username, token);
    let auth_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        auth_raw.as_bytes(),
    );
    let config = serde_json::json!({
        "auths": {
            "ghcr.io": {
                "auth": auth_b64
            }
        }
    });

    tokio::fs::write(&config_path, serde_json::to_string(&config)?).await?;
    let dir_path = dir.into_path().to_string_lossy().to_string();
    Ok((Some(dir_path), Some(auth_b64)))
}

async fn pull(cfg: &Config, version: &str, docker_config_dir: Option<&str>) -> Result<()> {
    info!(version = %version, "pulling images");
    compose(cfg, version, docker_config_dir, &["pull"]).await
}

async fn up(cfg: &Config, version: &str, docker_config_dir: Option<&str>) -> Result<()> {
    info!(version = %version, "restarting services");
    compose(cfg, version, docker_config_dir, &["up", "-d", "--remove-orphans"]).await
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

async fn compose(cfg: &Config, version: &str, docker_config_dir: Option<&str>, args: &[&str]) -> Result<()> {
    let mut cmd_args = vec!["compose", "-f", cfg.compose_file.as_str()];
    cmd_args.extend_from_slice(args);

    let mut cmd = Command::new("docker");
    cmd.args(&cmd_args)
        .env("GHCR_ORG", &cfg.ghcr_org)
        .env("CSF_VERSION", version)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(dir) = docker_config_dir {
        cmd.env("DOCKER_CONFIG", dir);
    }

    let status = cmd.status().await?;
    if !status.success() {
        bail!("docker compose {} failed: {}", args.join(" "), status);
    }
    Ok(())
}
