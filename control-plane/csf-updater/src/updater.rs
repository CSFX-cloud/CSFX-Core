use anyhow::{bail, Result};
use tracing::info;

use crate::config::Config;
use crate::etcd;

pub async fn run(cfg: &Config, flake_rev: &str, _etcd: &mut etcd::Client) -> Result<()> {
    nix_build(cfg, flake_rev).await?;
    nix_switch(cfg, flake_rev).await
}

async fn nix_build(cfg: &Config, flake_rev: &str) -> Result<()> {
    info!(flake_rev = %flake_rev, "running nix build");

    let flake_url = format!(
        "git+http://{}?rev={}",
        cfg.infra_repo_mirror_url, flake_rev
    );

    let status = tokio::process::Command::new("nixos-rebuild")
        .args(["build", "--flake", &flake_url])
        .status()
        .await?;

    if !status.success() {
        bail!("nix build failed for rev {}", flake_rev);
    }

    info!(flake_rev = %flake_rev, "nix build complete");
    Ok(())
}

async fn nix_switch(cfg: &Config, flake_rev: &str) -> Result<()> {
    info!(flake_rev = %flake_rev, "running nixos-rebuild switch");

    let flake_url = format!(
        "git+http://{}?rev={}",
        cfg.infra_repo_mirror_url, flake_rev
    );

    let status = tokio::process::Command::new("nixos-rebuild")
        .args(["switch", "--flake", &flake_url])
        .status()
        .await?;

    if !status.success() {
        bail!("nixos-rebuild switch failed for rev {}", flake_rev);
    }

    info!(flake_rev = %flake_rev, "nixos-rebuild switch complete");
    Ok(())
}
