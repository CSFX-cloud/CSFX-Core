use anyhow::{bail, Context, Result};
use tracing::{debug, error, info, warn};

use crate::apply::{self, Outcome};
use crate::config::Config;
use crate::etcd::{self, Client};
use crate::manifest::{self, Manifest};
use crate::{channel, nix, state, waves};

const LOCK_TTL_SECS: i64 = 300;
const ARCHES: [&str; 2] = ["x86_64-linux", "aarch64-linux"];

struct Signed {
    manifest: Manifest,
    raw: Vec<u8>,
    signature: String,
}

pub fn node_name() -> Result<String> {
    let machine_id =
        std::fs::read_to_string("/etc/machine-id").context("cannot read /etc/machine-id")?;
    Ok(format!(
        "csfx-{}",
        machine_id.trim().chars().take(8).collect::<String>()
    ))
}

fn verify_signed(cfg: &Config, raw: Vec<u8>, signature: String) -> Result<Signed> {
    let public_key = std::fs::read_to_string(&cfg.release_public_key_file)
        .with_context(|| format!("cannot read {}", cfg.release_public_key_file.display()))?;
    let manifest = manifest::verify(&raw, &signature, &public_key)?;
    Ok(Signed {
        manifest,
        raw,
        signature,
    })
}

async fn stored(cfg: &Config, etcd: &mut Client, arch: &str) -> Result<Option<Signed>> {
    let raw = etcd
        .get(&format!("{}{arch}", etcd::MANIFEST_PREFIX))
        .await?;
    let signature = etcd
        .get(&format!("{}{arch}", etcd::MANIFEST_SIGNATURE_PREFIX))
        .await?;
    match (raw, signature) {
        (Some(raw), Some(signature)) => Ok(Some(verify_signed(cfg, raw.into_bytes(), signature)?)),
        _ => Ok(None),
    }
}

async fn fetched(cfg: &Config, version: &str, arch: &str) -> Result<Signed> {
    let (raw, signature) = channel::fetch_manifest(cfg, version, arch).await?;
    verify_signed(cfg, raw, signature)
}

async fn locate(
    cfg: &Config,
    etcd: &mut Client,
    desired: &str,
    arch: &str,
) -> Result<Option<Signed>> {
    if desired == "latest" {
        return Ok(Some(fetched(cfg, desired, arch).await?));
    }
    let wanted = desired.trim_start_matches('v');
    Ok(stored(cfg, etcd, arch)
        .await?
        .filter(|signed| signed.manifest.version == wanted))
}

async fn publish(cfg: &Config, etcd: &mut Client, desired: &str, own_arch: &str) -> Result<Signed> {
    let mut own = None;
    for arch in ARCHES {
        match fetched(cfg, desired, arch).await {
            Ok(signed) => {
                store(
                    etcd,
                    arch,
                    &signed,
                    etcd::MANIFEST_PREFIX,
                    etcd::MANIFEST_SIGNATURE_PREFIX,
                )
                .await?;
                if arch == own_arch {
                    own = Some(signed);
                }
            }
            Err(error) if arch != own_arch => {
                warn!(%arch, %error, "manifest for other architecture unavailable")
            }
            Err(error) => return Err(error),
        }
    }
    own.context("manifest for this architecture missing")
}

async fn store(
    etcd: &mut Client,
    arch: &str,
    signed: &Signed,
    manifest_prefix: &str,
    signature_prefix: &str,
) -> Result<()> {
    etcd.put(
        &format!("{manifest_prefix}{arch}"),
        std::str::from_utf8(&signed.raw)?,
    )
    .await?;
    etcd.put(&format!("{signature_prefix}{arch}"), &signed.signature)
        .await
}

async fn mark_applied(etcd: &mut Client, node: &str, version: &str) -> Result<()> {
    etcd.put(&format!("{}{node}", etcd::APPLIED_PREFIX), version)
        .await?;
    release_to_workers(etcd, version).await
}

async fn release_to_workers(etcd: &mut Client, version: &str) -> Result<()> {
    let applied = etcd.values_with_prefix(etcd::APPLIED_PREFIX).await?;
    let masters = etcd.member_count().await?;
    if applied
        .iter()
        .filter(|applied| applied.as_str() == version)
        .count()
        < masters
    {
        return Ok(());
    }
    waves::begin(etcd, version).await?;
    for arch in ARCHES {
        let raw = etcd
            .get(&format!("{}{arch}", etcd::MANIFEST_PREFIX))
            .await?;
        let signature = etcd
            .get(&format!("{}{arch}", etcd::MANIFEST_SIGNATURE_PREFIX))
            .await?;
        if let (Some(raw), Some(signature)) = (raw, signature) {
            etcd.put(&format!("{}{arch}", etcd::WORKER_MANIFEST_PREFIX), &raw)
                .await?;
            etcd.put(
                &format!("{}{arch}", etcd::WORKER_MANIFEST_SIGNATURE_PREFIX),
                &signature,
            )
            .await?;
        }
    }
    Ok(())
}

async fn record_outcome(
    etcd: &mut Client,
    node: &str,
    manifest: &Manifest,
    outcome: &Outcome,
) -> Result<()> {
    match outcome {
        Outcome::Applied | Outcome::AlreadyCurrent => {
            mark_applied(etcd, node, &manifest.version).await?;
            etcd.put(etcd::BUILD_STATUS_KEY, "ready").await?;
            etcd.put(etcd::RESULT_KEY, "success").await
        }
        Outcome::RolledBack => {
            etcd.put(etcd::BUILD_STATUS_KEY, "rolled_back").await?;
            etcd.put(etcd::RESULT_KEY, "rolled_back").await
        }
    }
}

async fn record_failure(etcd: &mut Client) -> Result<()> {
    etcd.put(etcd::BUILD_STATUS_KEY, "failed").await?;
    etcd.put(etcd::RESULT_KEY, "failed").await
}

async fn install(
    cfg: &Config,
    etcd: &mut Client,
    node: &str,
    signed: &Signed,
    forced: bool,
) -> Result<()> {
    let staging = cfg.state_dir.join("staging");
    tokio::fs::create_dir_all(&staging).await?;
    let manifest_path = staging.join(format!("manifest-{}.json", signed.manifest.arch));
    tokio::fs::write(&manifest_path, &signed.raw).await?;
    tokio::fs::write(
        format!("{}.sig", manifest_path.display()),
        &signed.signature,
    )
    .await?;
    etcd.put(etcd::BUILD_STATUS_KEY, "building").await?;
    match apply::apply(cfg, &manifest_path, forced).await {
        Ok(outcome) => record_outcome(etcd, node, &signed.manifest, &outcome).await,
        Err(error) => {
            error!(%error, version = %signed.manifest.version, "update failed");
            record_failure(etcd).await
        }
    }
}

async fn update_under_lock(
    cfg: &Config,
    etcd: &mut Client,
    node: &str,
    desired: &str,
    forced: bool,
) -> Result<()> {
    let arch = manifest::host_arch();
    let signed = match locate(cfg, etcd, desired, &arch).await? {
        Some(signed) => signed,
        None => publish(cfg, etcd, desired, &arch).await?,
    };
    if signed.manifest.version != desired.trim_start_matches('v') && desired != "latest" {
        bail!(
            "channel served version {} instead of {desired}",
            signed.manifest.version
        );
    }
    info!(version = %signed.manifest.version, toplevel = %signed.manifest.toplevel, "starting update");
    install(cfg, etcd, node, &signed, forced).await
}

async fn already_current(
    cfg: &Config,
    etcd: &mut Client,
    node: &str,
    desired: &str,
) -> Result<bool> {
    let arch = manifest::host_arch();
    let Some(signed) = locate(cfg, etcd, desired, &arch).await? else {
        return Ok(false);
    };
    if signed.manifest.toplevel != nix::current_system().await? {
        return Ok(false);
    }
    mark_applied(etcd, node, &signed.manifest.version).await?;
    Ok(true)
}

async fn resume(cfg: &Config, etcd: &mut Client, node: &str) -> Result<()> {
    let lock = etcd
        .acquire_lock(etcd::UPDATE_LOCK_KEY, node, LOCK_TTL_SECS)
        .await?;
    let pending = state::load_pending(&cfg.state_dir)?.context("pending state vanished")?;
    info!(version = %pending.manifest.version, "resuming interrupted update");
    let outcome = apply::finish_pending(cfg).await;
    let recorded = match &outcome {
        Ok(outcome) => record_outcome(etcd, node, &pending.manifest, outcome).await,
        Err(_) => record_failure(etcd).await,
    };
    if let Some(lock) = lock {
        lock.release().await?;
    }
    outcome?;
    recorded
}

async fn drive_rollout(cfg: &Config, etcd: &mut Client, node: &str) -> Result<()> {
    let Some(lock) = etcd
        .acquire_lock(etcd::ROLLOUT_LOCK_KEY, node, LOCK_TTL_SECS)
        .await?
    else {
        return Ok(());
    };
    let result = waves::advance(cfg, etcd).await;
    lock.release().await?;
    result
}

pub async fn run_cycle(cfg: &Config) -> Result<()> {
    let node = node_name()?;
    let mut etcd = Client::connect(cfg).await?;
    drive_rollout(cfg, &mut etcd, &node).await?;
    if state::load_pending(&cfg.state_dir)?.is_some() {
        return resume(cfg, &mut etcd, &node).await;
    }
    let forced = etcd.get(etcd::FORCE_UPDATE_KEY).await?.as_deref() == Some("true");
    if !forced && etcd.get(etcd::PAUSED_KEY).await?.as_deref() == Some("true") {
        debug!("updater paused, skipping cycle");
        return Ok(());
    }
    let desired = match etcd.get(etcd::DESIRED_VERSION_KEY).await? {
        Some(version) if !version.is_empty() => version,
        _ => return Ok(()),
    };
    if already_current(cfg, &mut etcd, &node, &desired).await? {
        return Ok(());
    }
    let Some(lock) = etcd
        .acquire_lock(etcd::UPDATE_LOCK_KEY, &node, LOCK_TTL_SECS)
        .await?
    else {
        debug!("another master holds the update lock");
        return Ok(());
    };
    if forced {
        etcd.delete(etcd::FORCE_UPDATE_KEY).await?;
    }
    let result = lock
        .run_while_held(update_under_lock(cfg, &mut etcd, &node, &desired, forced))
        .await;
    lock.release().await?;
    result
}
