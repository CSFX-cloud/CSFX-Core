use anyhow::{bail, Result};
use tokio::process::Command;
use tokio::sync::watch;
use tracing::info;

pub async fn build(mirror_dir: &str, rev: &str, nixos_config: &str, mut cancel: watch::Receiver<bool>) -> Result<()> {
    let flake_url = format!("git+file://{}?rev={}#{}", mirror_dir, rev, nixos_config);

    info!(flake_rev = %rev, nixos_config = %nixos_config, "starting nix build");

    let mut child = Command::new("nixos-rebuild")
        .args(["build", "--no-out-link", "--flake", &flake_url])
        .spawn()?;

    tokio::select! {
        result = child.wait() => {
            let status = result?;
            if !status.success() {
                bail!("nix build failed for rev {}", rev);
            }
            info!(flake_rev = %rev, "nix build complete");
            Ok(())
        }
        _ = cancel.changed() => {
            if *cancel.borrow() {
                let _ = child.kill().await;
                bail!("nix build cancelled for rev {}", rev);
            }
            let status = child.wait().await?;
            if !status.success() {
                bail!("nix build failed for rev {}", rev);
            }
            Ok(())
        }
    }
}
