use anyhow::{bail, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::info;

pub async fn sync(mirror_dir: &str, remote_url: &str) -> Result<()> {
    if Path::new(mirror_dir).join("HEAD").exists() {
        fetch(mirror_dir).await
    } else {
        clone(mirror_dir, remote_url).await
    }
}

async fn clone(mirror_dir: &str, remote_url: &str) -> Result<()> {
    info!(mirror_dir = %mirror_dir, remote_url = %remote_url, "cloning infra repo mirror");

    let status = Command::new("git")
        .args(["clone", "--mirror", remote_url, mirror_dir])
        .status()
        .await?;

    if !status.success() {
        bail!("git clone --mirror failed for {}", remote_url);
    }

    info!(mirror_dir = %mirror_dir, "mirror clone complete");
    Ok(())
}

async fn fetch(mirror_dir: &str) -> Result<()> {
    info!(mirror_dir = %mirror_dir, "fetching infra repo mirror");

    let status = Command::new("git")
        .args(["--git-dir", mirror_dir, "fetch", "--prune"])
        .status()
        .await?;

    if !status.success() {
        bail!("git fetch --prune failed in {}", mirror_dir);
    }

    info!(mirror_dir = %mirror_dir, "mirror fetch complete");
    Ok(())
}

pub async fn rev_exists(mirror_dir: &str, rev: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["--git-dir", mirror_dir, "cat-file", "-t", rev])
        .output()
        .await?;

    Ok(output.status.success()
        && String::from_utf8_lossy(&output.stdout).trim() == "commit")
}
