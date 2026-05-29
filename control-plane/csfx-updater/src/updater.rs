use anyhow::{bail, Result};
use tokio::process::Command;
use tracing::info;

use crate::config::Config;

pub async fn switch(cfg: &Config, flake_rev: &str) -> Result<()> {
    let flake_url = format!(
        "git+file://{}?rev={}#{}",
        cfg.infra_repo_mirror_dir, flake_rev, cfg.nixos_config
    );

    info!(flake_rev = %flake_rev, "running nixos-rebuild switch");

    let mut child = Command::new("nixos-rebuild")
        .args(["switch", "--flake", &flake_url])
        .spawn()?;

    info!(flake_rev = %flake_rev, pid = child.id(), "nixos-rebuild switch spawned, activating configuration");

    let status = child.wait().await?;

    if !status.success() {
        bail!("nixos-rebuild switch failed for rev {}", flake_rev);
    }

    info!(flake_rev = %flake_rev, "nixos-rebuild switch complete");
    Ok(())
}
