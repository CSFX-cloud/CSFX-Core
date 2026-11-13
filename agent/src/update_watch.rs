use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

use crate::client::SignedRelease;

const UPDATE_DIR: &str = "/var/lib/csfx/update";
const MANIFEST_FILE: &str = "manifest.json";
const SIGNATURE_FILE: &str = "manifest.json.sig";
const MAX_JITTER_SECS: u64 = 300;
const APPLIED_FILE: &str = "/var/lib/csfx-updater/applied.json";

static STAGING_PENDING: AtomicBool = AtomicBool::new(false);

struct StagingClaim;

impl StagingClaim {
    fn acquire() -> Option<Self> {
        (!STAGING_PENDING.swap(true, Ordering::AcqRel)).then_some(Self)
    }
}

impl Drop for StagingClaim {
    fn drop(&mut self) {
        STAGING_PENDING.store(false, Ordering::Release);
    }
}

pub async fn applied_version() -> Option<String> {
    let raw = fs::read(APPLIED_FILE).await.ok()?;
    parse_applied_version(&raw)
}

fn parse_applied_version(raw: &[u8]) -> Option<String> {
    let manifest: serde_json::Value = serde_json::from_slice(raw).ok()?;
    manifest.get("version")?.as_str().map(str::to_string)
}

pub async fn handle(agent_id: Uuid, releases: HashMap<String, SignedRelease>) {
    let Some(release) = releases.get(&host_arch()) else {
        return;
    };
    let directory = Path::new(UPDATE_DIR);
    if is_staged(directory, release).await {
        return;
    }

    let Some(_claim) = StagingClaim::acquire() else {
        return;
    };

    let jitter = jitter_delay(agent_id);
    info!(
        jitter_secs = jitter,
        "release received, waiting before staging"
    );
    tokio::time::sleep(Duration::from_secs(jitter)).await;

    match stage(directory, release).await {
        Ok(()) => info!("release staged"),
        Err(error) => warn!(%error, "failed to stage release"),
    }
}

fn host_arch() -> String {
    format!("{}-linux", std::env::consts::ARCH)
}

async fn is_staged(directory: &Path, release: &SignedRelease) -> bool {
    fs::read_to_string(directory.join(MANIFEST_FILE))
        .await
        .is_ok_and(|staged| staged == release.manifest)
}

async fn stage(directory: &Path, release: &SignedRelease) -> anyhow::Result<()> {
    fs::create_dir_all(directory).await?;
    write_atomic(&directory.join(SIGNATURE_FILE), &release.signature).await?;
    write_atomic(&directory.join(MANIFEST_FILE), &release.manifest).await
}

async fn write_atomic(path: &Path, content: &str) -> anyhow::Result<()> {
    let staging = path.with_extension("tmp");
    fs::write(&staging, content).await?;
    fs::rename(&staging, path).await?;
    Ok(())
}

fn jitter_delay(agent_id: Uuid) -> u64 {
    let bytes = agent_id.as_bytes();
    let val = u64::from_le_bytes(bytes[..8].try_into().unwrap_or([0u8; 8]));
    val % MAX_JITTER_SECS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(manifest: &str) -> SignedRelease {
        SignedRelease {
            manifest: manifest.to_string(),
            signature: "sig".to_string(),
        }
    }

    #[test]
    fn jitter_is_deterministic_and_bounded() {
        let id = Uuid::from_u128(0xdead_beef_1234_5678_9abc_def0_1122_3344);
        assert_eq!(jitter_delay(id), jitter_delay(id));
        assert!(jitter_delay(id) < MAX_JITTER_SECS);
        assert_eq!(jitter_delay(Uuid::nil()), 0);
    }

    #[test]
    fn only_one_staging_claim_exists_at_a_time() {
        let first = StagingClaim::acquire();
        assert!(first.is_some());
        assert!(StagingClaim::acquire().is_none());
        drop(first);
        assert!(StagingClaim::acquire().is_some());
    }

    #[test]
    fn applied_version_is_read_from_the_manifest() {
        assert_eq!(
            parse_applied_version(br#"{"version":"0.3.0","arch":"x"}"#).as_deref(),
            Some("0.3.0")
        );
        assert_eq!(parse_applied_version(b"{}"), None);
        assert_eq!(parse_applied_version(b"garbage"), None);
    }

    #[tokio::test]
    async fn staging_writes_both_files_and_detects_unchanged_manifests() {
        let directory = std::env::temp_dir().join(format!("csfx-stage-{}", Uuid::new_v4()));
        assert!(!is_staged(&directory, &release("m1")).await);
        stage(&directory, &release("m1")).await.unwrap();
        assert!(is_staged(&directory, &release("m1")).await);
        assert!(!is_staged(&directory, &release("m2")).await);
        assert_eq!(
            fs::read_to_string(directory.join(SIGNATURE_FILE))
                .await
                .unwrap(),
            "sig"
        );
        fs::remove_dir_all(&directory).await.unwrap();
    }

    #[tokio::test]
    async fn handle_ignores_releases_for_other_architectures() {
        let releases = HashMap::from([("riscv64-linux".to_string(), release("m"))]);
        handle(Uuid::new_v4(), releases).await;
    }
}
