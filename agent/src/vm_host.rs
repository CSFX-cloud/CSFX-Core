use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn};

use crate::cmd;
use crate::runtime::Runtime;

const SIDECAR_FILE: &str = "csfx-meta.json";

pub fn process_uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn process_gid() -> u32 {
    unsafe { libc::getgid() }
}

pub async fn create_tap(
    tap_device: &str,
    bridge_iface: Option<&str>,
    owner_uid: u32,
) -> Result<()> {
    let uid = owner_uid.to_string();
    cmd::ip(&[
        "tuntap", "add", "dev", tap_device, "mode", "tap", "user", &uid,
    ])
    .await
    .with_context(|| format!("failed to create tap device {tap_device}"))?;

    if let Some(bridge) = bridge_iface {
        cmd::ip(&["link", "set", "dev", tap_device, "master", bridge])
            .await
            .with_context(|| {
                format!("failed to attach tap device {tap_device} to bridge {bridge}")
            })?;
    }

    cmd::ip(&["link", "set", "dev", tap_device, "up"])
        .await
        .with_context(|| format!("failed to bring up tap device {tap_device}"))
}

pub async fn remove_tap(tap_device: &str) -> Result<()> {
    cmd::delete_link(tap_device)
        .await
        .with_context(|| format!("failed to delete tap device {tap_device}"))
}

pub fn sidecar_path(dir: &Path) -> PathBuf {
    dir.join(SIDECAR_FILE)
}

pub async fn write_sidecar<T: Serialize>(dir: &Path, metadata: &T) -> Result<()> {
    let payload = serde_json::to_vec(metadata).context("Failed to serialize sidecar metadata")?;
    tokio::fs::write(sidecar_path(dir), payload)
        .await
        .context("Failed to write sidecar metadata file")
}

pub async fn read_sidecar<T: DeserializeOwned>(dir: &Path) -> Option<T> {
    let content = tokio::fs::read(sidecar_path(dir)).await.ok()?;
    serde_json::from_slice(&content).ok()
}

pub async fn recover_once(
    runtime: &impl Runtime,
    reconciled: &AtomicBool,
    kind: &str,
) -> Vec<(String, String)> {
    if reconciled.swap(true, Ordering::SeqCst) {
        return Vec::new();
    }

    match runtime.list_managed_workloads().await {
        Ok(recovered) => {
            info!(
                count = recovered.len(),
                kind, "Reconciled running workloads from disk state"
            );
            recovered
        }
        Err(e) => {
            warn!(kind, error = %e, "Failed to reconcile workloads from disk state");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeTools;

    #[test]
    fn process_identity_matches_the_kernel() {
        assert_eq!(process_uid(), unsafe { libc::getuid() });
        assert_eq!(process_gid(), unsafe { libc::getgid() });
    }

    #[tokio::test]
    async fn tap_devices_are_created_attached_and_raised() {
        let tools = FakeTools::install(&[("ip", "exit 0")]);

        create_tap("tap1", Some("br0"), 77).await.unwrap();

        assert_eq!(
            tools.calls("ip"),
            vec![
                "tuntap add dev tap1 mode tap user 77".to_string(),
                "link set dev tap1 master br0".to_string(),
                "link set dev tap1 up".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn tap_devices_without_a_bridge_skip_attachment() {
        let tools = FakeTools::install(&[("ip", "exit 0")]);

        create_tap("tap1", None, 0).await.unwrap();

        assert_eq!(tools.calls("ip").len(), 2);
    }

    #[tokio::test]
    async fn existing_tap_devices_are_tolerated() {
        let _tools = FakeTools::install(&[(
            "ip",
            r#"case "$*" in tuntap*) echo 'ioctl: File exists' >&2; exit 1;; esac"#,
        )]);

        create_tap("tap1", None, 0).await.unwrap();
    }

    #[tokio::test]
    async fn tap_device_failures_name_the_failing_step() {
        {
            let _tools = FakeTools::install(&[(
                "ip",
                r#"case "$*" in tuntap*) echo 'Operation not permitted' >&2; exit 1;; esac"#,
            )]);
            let error = create_tap("tap1", None, 0).await.unwrap_err().to_string();
            assert!(error.contains("failed to create tap device tap1"));
        }
        {
            let _tools = FakeTools::install(&[("ip", r#"case "$*" in *master*) exit 1;; esac"#)]);
            let error = create_tap("tap1", Some("br0"), 0)
                .await
                .unwrap_err()
                .to_string();
            assert!(error.contains("failed to attach tap device tap1 to bridge br0"));
        }
        let _tools = FakeTools::install(&[("ip", r#"case "$*" in *" up") exit 1;; esac"#)]);
        let error = create_tap("tap1", None, 0).await.unwrap_err().to_string();
        assert!(error.contains("failed to bring up tap device tap1"));
    }

    #[tokio::test]
    async fn tap_devices_are_removed_by_name() {
        {
            let tools = FakeTools::install(&[("ip", "exit 0")]);
            remove_tap("tap1").await.unwrap();
            assert_eq!(tools.calls("ip"), vec!["link delete tap1".to_string()]);
        }
        let _tools = FakeTools::install(&[("ip", "echo 'Operation not permitted' >&2\nexit 1")]);
        assert!(remove_tap("tap1").await.is_err());
    }

    #[derive(Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Sample {
        value: u32,
    }

    #[tokio::test]
    async fn sidecar_metadata_round_trips() {
        let dir = std::env::temp_dir().join(format!("csfx-sidecar-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_sidecar::<Sample>(&dir).await.is_none());

        write_sidecar(&dir, &Sample { value: 7 }).await.unwrap();

        assert_eq!(
            read_sidecar::<Sample>(&dir).await,
            Some(Sample { value: 7 })
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
