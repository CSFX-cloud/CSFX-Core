use anyhow::{bail, Context, Result};
use std::path::Path;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::manifest::{self, Manifest};
use crate::state::{self, Pending};
use crate::version::Version;
use crate::{channel, health, nix};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    AlreadyCurrent,
    Applied,
    RolledBack,
}

pub fn load_verified(cfg: &Config, manifest_path: &Path) -> Result<Manifest> {
    let raw = std::fs::read(manifest_path)
        .with_context(|| format!("cannot read {}", manifest_path.display()))?;
    let signature_path = format!("{}.sig", manifest_path.display());
    let signature = std::fs::read_to_string(&signature_path)
        .with_context(|| format!("cannot read {signature_path}"))?;
    let public_key = std::fs::read_to_string(&cfg.release_public_key_file)
        .with_context(|| format!("cannot read {}", cfg.release_public_key_file.display()))?;
    manifest::verify(&raw, &signature, &public_key)
}

fn guard_applicable(cfg: &Config, manifest: &Manifest, force: bool) -> Result<()> {
    if manifest.arch != manifest::host_arch() {
        bail!(
            "manifest is for {} but this node is {}",
            manifest.arch,
            manifest::host_arch()
        );
    }
    let Some(applied) = state::load_applied(&cfg.state_dir)? else {
        return Ok(());
    };
    let downgrade = Version::parse(&manifest.version)? < Version::parse(&applied.version)?;
    if downgrade && !force {
        bail!(
            "downgrade from {} to {} refused",
            applied.version,
            manifest.version
        );
    }
    Ok(())
}

pub async fn apply(cfg: &Config, manifest_path: &Path, force: bool) -> Result<Outcome> {
    let manifest = load_verified(cfg, manifest_path)?;
    let previous = nix::current_system().await?;
    if manifest.toplevel == previous {
        state::save_applied(&cfg.state_dir, &manifest)?;
        return Ok(Outcome::AlreadyCurrent);
    }
    guard_applicable(cfg, &manifest, force)?;
    let manifest_dir = manifest_path.parent().unwrap_or(Path::new("."));
    fetch_closure(cfg, &manifest, manifest_dir).await?;

    let pending = Pending {
        previous_toplevel: previous,
        manifest,
    };
    state::save_pending(&cfg.state_dir, &pending)?;
    info!(version = %pending.manifest.version, toplevel = %pending.manifest.toplevel, "activating system closure");
    nix::set_profile(&pending.manifest.toplevel).await?;
    if let Err(error) = nix::switch(&pending.manifest.toplevel).await {
        error!(%error, "activation failed");
        return rollback(cfg, &pending).await;
    }
    finish_pending(cfg).await
}

pub async fn finish_pending(cfg: &Config) -> Result<Outcome> {
    nix::wait_until_idle().await;
    let pending = state::load_pending(&cfg.state_dir)?.context("no pending update to finish")?;
    let running = nix::current_system().await?;
    if running == pending.manifest.toplevel && health::check(cfg).await {
        state::save_applied(&cfg.state_dir, &pending.manifest)?;
        state::clear_pending(&cfg.state_dir)?;
        info!(version = %pending.manifest.version, "update verified and complete");
        return Ok(Outcome::Applied);
    }
    rollback(cfg, &pending).await
}

async fn rollback(cfg: &Config, pending: &Pending) -> Result<Outcome> {
    warn!(previous = %pending.previous_toplevel, "rolling back to previous system");
    nix::set_profile(&pending.previous_toplevel).await?;
    nix::switch_back(&pending.previous_toplevel).await?;
    state::clear_pending(&cfg.state_dir)?;
    Ok(Outcome::RolledBack)
}

fn substituters(cfg: &Config, bundle_dir: Option<&Path>) -> Vec<String> {
    ordered_sources(
        &cfg.peer_substituters,
        &cfg.upstream_substituters,
        bundle_dir,
    )
}

fn ordered_sources(
    peers: &[String],
    upstream: &[String],
    bundle_dir: Option<&Path>,
) -> Vec<String> {
    bundle_dir
        .map(|dir| format!("file://{}", dir.display()))
        .into_iter()
        .chain(peers.iter().cloned())
        .chain(upstream.iter().cloned())
        .collect()
}

async fn fetch_closure(cfg: &Config, manifest: &Manifest, manifest_dir: &Path) -> Result<()> {
    let local_bundle = manifest_dir.join(&manifest.bundle);
    if local_bundle.is_file() {
        return realise_from_bundle(cfg, manifest, &local_bundle).await;
    }
    match nix::realise(&manifest.toplevel, &substituters(cfg, None)).await {
        Ok(()) => Ok(()),
        Err(error) => {
            warn!(%error, "closure unavailable from peers and caches, downloading release bundle");
            download_and_realise(cfg, manifest).await
        }
    }
}

async fn download_and_realise(cfg: &Config, manifest: &Manifest) -> Result<()> {
    let bundle = cfg.state_dir.join(&manifest.bundle);
    tokio::fs::create_dir_all(&cfg.state_dir).await?;
    channel::download_bundle(cfg, &manifest.version, &manifest.bundle, &bundle).await?;
    let result = realise_from_bundle(cfg, manifest, &bundle).await;
    tokio::fs::remove_file(&bundle).await?;
    result
}

async fn realise_from_bundle(cfg: &Config, manifest: &Manifest, bundle: &Path) -> Result<()> {
    let extracted = cfg.state_dir.join("bundle");
    if extracted.exists() {
        tokio::fs::remove_dir_all(&extracted).await?;
    }
    channel::extract_bundle(bundle, &extracted).await?;
    let result = nix::realise(&manifest.toplevel, &substituters(cfg, Some(&extracted))).await;
    tokio::fs::remove_dir_all(&extracted).await?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_preferred_over_peers_over_upstream() {
        let peers = vec!["http://10.0.0.1:5000".to_string()];
        let upstream = vec!["https://cache.nixos.org".to_string()];
        let order = ordered_sources(&peers, &upstream, Some(Path::new("/tmp/bundle")));
        assert_eq!(
            order,
            vec![
                "file:///tmp/bundle",
                "http://10.0.0.1:5000",
                "https://cache.nixos.org"
            ]
        );
    }
}
