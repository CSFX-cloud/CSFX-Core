use anyhow::{anyhow, Context, Result};
use std::time::Duration;
use tokio::process::Command;
use tracing::info;

const MAP_LOCK_TIMEOUT_SECS: u64 = 30;
const MAP_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const BLKID_NO_SIGNATURE_EXIT_CODE: i32 = 2;

pub struct MappedImage {
    pub pool: String,
    pub image: String,
    pub device: String,
}

struct CephClientArgs {
    mon_hosts: String,
    keyring_path: Option<String>,
    client_name: String,
}

impl CephClientArgs {
    fn from_env() -> Self {
        Self {
            mon_hosts: std::env::var("CEPH_MON_HOSTS")
                .unwrap_or_else(|_| "ceph-mon1:6789,ceph-mon2:6789,ceph-mon3:6789".to_string()),
            keyring_path: std::env::var("CEPH_KEYRING").ok(),
            client_name: std::env::var("CEPH_CLIENT_NAME").unwrap_or_else(|_| "admin".to_string()),
        }
    }

    fn apply(&self, command: &mut Command) {
        command.arg("-m").arg(&self.mon_hosts);
        if let Some(ref keyring) = self.keyring_path {
            command.arg("--keyring").arg(keyring);
        }
        command
            .arg("--name")
            .arg(format!("client.{}", self.client_name));
    }
}

pub async fn map_device(pool: &str, image: &str) -> Result<String> {
    info!(pool = %pool, image = %image, "Mapping RBD device");

    let ceph_args = CephClientArgs::from_env();
    let mut command = Command::new("rbd");
    command
        .arg("map")
        .arg(format!("{}/{}", pool, image))
        .arg("-o")
        .arg(format!("exclusive,lock_timeout={}", MAP_LOCK_TIMEOUT_SECS))
        .kill_on_drop(true);
    ceph_args.apply(&mut command);

    let output = tokio::time::timeout(MAP_COMMAND_TIMEOUT, command.output())
        .await
        .context("rbd map timed out")?
        .context("Failed to execute rbd map")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("rbd map failed: {}", stderr));
    }

    let device = String::from_utf8(output.stdout)
        .context("Invalid rbd map output")?
        .trim()
        .trim_matches('"')
        .to_string();

    info!(device = %device, "RBD device mapped");
    Ok(device)
}

pub async fn unmap_device(device: &str) -> Result<()> {
    info!(device = %device, "Unmapping RBD device");

    let ceph_args = CephClientArgs::from_env();
    let mut command = Command::new("rbd");
    command.arg("unmap").arg(device).kill_on_drop(true);
    ceph_args.apply(&mut command);

    let output = tokio::time::timeout(MAP_COMMAND_TIMEOUT, command.output())
        .await
        .context("rbd unmap timed out")?
        .context("Failed to execute rbd unmap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("rbd unmap failed: {}", stderr));
    }

    Ok(())
}

pub async fn list_mapped() -> Result<Vec<MappedImage>> {
    let output = Command::new("rbd")
        .args(["showmapped", "--format", "json"])
        .output()
        .await
        .context("Failed to execute rbd showmapped")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("rbd showmapped failed: {}", stderr));
    }

    parse_showmapped(&output.stdout)
}

fn parse_showmapped(raw: &[u8]) -> Result<Vec<MappedImage>> {
    let parsed: serde_json::Value =
        serde_json::from_slice(raw).context("Invalid rbd showmapped output")?;

    let entries: Vec<&serde_json::Value> = match &parsed {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Object(items) => items.values().collect(),
        _ => return Err(anyhow!("Unexpected rbd showmapped output shape")),
    };

    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            Some(MappedImage {
                pool: entry["pool"].as_str()?.to_string(),
                image: entry["name"].as_str()?.to_string(),
                device: entry["device"].as_str()?.to_string(),
            })
        })
        .collect())
}

pub async fn ensure_filesystem(device: &str, label: &str) -> Result<()> {
    let probe = Command::new("blkid")
        .args(["-p", device])
        .output()
        .await
        .context("Failed to execute blkid")?;

    match probe.status.code() {
        Some(0) => return Ok(()),
        Some(BLKID_NO_SIGNATURE_EXIT_CODE) => {}
        _ => {
            let stderr = String::from_utf8_lossy(&probe.stderr);
            return Err(anyhow!("blkid probe failed device={} {}", device, stderr));
        }
    }

    info!(device = %device, label = %label, "Formatting empty volume");

    let format = Command::new("mkfs.ext4")
        .args(["-q", "-L", label, device])
        .output()
        .await
        .context("Failed to execute mkfs.ext4")?;

    if !format.status.success() {
        let stderr = String::from_utf8_lossy(&format.stderr);
        return Err(anyhow!("mkfs.ext4 failed device={} {}", device, stderr));
    }

    Ok(())
}

const BYTES_PER_GB: u64 = 1_073_741_824;
const MB_PER_GB: u64 = 1024;
const E2FSCK_MAX_CORRECTED_EXIT_CODE: i32 = 1;

async fn device_bytes(device: &str) -> Result<u64> {
    let output = Command::new("blockdev")
        .args(["--getsize64", device])
        .output()
        .await
        .context("Failed to execute blockdev")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("blockdev failed device={} {}", device, stderr));
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .context("Invalid blockdev output")
}

pub async fn grow_image_to(pool: &str, image: &str, device: &str, size_gb: u64) -> Result<bool> {
    if device_bytes(device).await? >= size_gb * BYTES_PER_GB {
        return Ok(false);
    }

    info!(pool = %pool, image = %image, size_gb = size_gb, "Growing RBD image");

    let mut command = Command::new("rbd");
    command
        .arg("resize")
        .arg(format!("{}/{}", pool, image))
        .arg("--size")
        .arg((size_gb * MB_PER_GB).to_string())
        .kill_on_drop(true);
    CephClientArgs::from_env().apply(&mut command);

    let output = tokio::time::timeout(MAP_COMMAND_TIMEOUT, command.output())
        .await
        .context("rbd resize timed out")?
        .context("Failed to execute rbd resize")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("rbd resize failed: {}", stderr));
    }

    Ok(true)
}

const FILESYSTEM_TOOL_TIMEOUT: Duration = Duration::from_secs(600);
const FILESYSTEM_SANDBOX: [&str; 22] = [
    "DynamicUser=yes",
    "SupplementaryGroups=disk",
    "CapabilityBoundingSet=",
    "NoNewPrivileges=yes",
    "PrivateNetwork=yes",
    "PrivateTmp=yes",
    "ProtectSystem=strict",
    "ProtectHome=yes",
    "ProtectKernelTunables=yes",
    "ProtectKernelModules=yes",
    "ProtectKernelLogs=yes",
    "ProtectControlGroups=yes",
    "ProtectClock=yes",
    "RestrictNamespaces=yes",
    "RestrictRealtime=yes",
    "LockPersonality=yes",
    "MemoryDenyWriteExecute=yes",
    "SystemCallArchitectures=native",
    "SystemCallFilter=@system-service",
    "DevicePolicy=closed",
    "MemoryMax=1G",
    "RuntimeMaxSec=600",
];

fn sandboxed(device: &str, program: &str, args: &[&str]) -> Command {
    let mut command = Command::new("systemd-run");
    command.args(["--quiet", "--wait", "--collect", "--pipe"]);
    for property in FILESYSTEM_SANDBOX {
        command.arg("-p").arg(property);
    }
    command
        .arg("-p")
        .arg(format!("DeviceAllow={device} rw"))
        .arg("--")
        .arg(program)
        .args(args)
        .kill_on_drop(true);
    command
}

async fn run_sandboxed(device: &str, program: &str, args: &[&str]) -> Result<std::process::Output> {
    tokio::time::timeout(
        FILESYSTEM_TOOL_TIMEOUT,
        sandboxed(device, program, args).output(),
    )
    .await
    .with_context(|| format!("{program} timed out"))?
    .with_context(|| format!("Failed to execute {program} in its sandbox"))
}

pub async fn grow_filesystem(device: &str) -> Result<()> {
    let check = run_sandboxed(device, "e2fsck", &["-fp", device]).await?;

    if check
        .status
        .code()
        .is_none_or(|code| code > E2FSCK_MAX_CORRECTED_EXIT_CODE)
    {
        let stderr = String::from_utf8_lossy(&check.stderr);
        return Err(anyhow!("e2fsck failed device={} {}", device, stderr));
    }

    let resize = run_sandboxed(device, "resize2fs", &[device]).await?;

    if !resize.status.success() {
        let stderr = String::from_utf8_lossy(&resize.stderr);
        return Err(anyhow!("resize2fs failed device={} {}", device, stderr));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_array_output() {
        let raw = br#"[{"id":"0","pool":"csfx","namespace":"","name":"vol-1","snap":"-","device":"/dev/rbd0"}]"#;
        let mapped = parse_showmapped(raw).unwrap();
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].pool, "csfx");
        assert_eq!(mapped[0].image, "vol-1");
        assert_eq!(mapped[0].device, "/dev/rbd0");
    }

    #[test]
    fn parses_legacy_object_output() {
        let raw = br#"{"0":{"pool":"csfx","name":"vol-1","snap":"-","device":"/dev/rbd0"}}"#;
        let mapped = parse_showmapped(raw).unwrap();
        assert_eq!(mapped[0].device, "/dev/rbd0");
    }

    #[test]
    fn rejects_invalid_output() {
        assert!(parse_showmapped(b"not json").is_err());
        assert!(parse_showmapped(b"42").is_err());
    }

    use crate::testing::FakeTools;

    #[tokio::test]
    async fn mapping_returns_the_trimmed_device_and_passes_ceph_arguments() {
        let tools = FakeTools::install(&[("rbd", "echo '\"/dev/rbd3\"'")]);

        let device = map_device("csfx", "vol-1").await.unwrap();

        assert_eq!(device, "/dev/rbd3");
        let call = &tools.calls("rbd")[0];
        assert!(call.starts_with("map csfx/vol-1 -o exclusive,lock_timeout=30 -m "));
        assert!(call.contains("--name client."));
    }

    #[tokio::test]
    async fn failed_mapping_reports_the_error_output() {
        let _tools = FakeTools::install(&[("rbd", "echo 'no such image' >&2\nexit 1")]);

        let error = map_device("csfx", "vol-1").await.unwrap_err().to_string();

        assert!(error.contains("rbd map failed"));
        assert!(error.contains("no such image"));
    }

    #[tokio::test]
    async fn unmapping_passes_the_device_and_surfaces_failures() {
        {
            let tools = FakeTools::install(&[("rbd", "exit 0")]);
            unmap_device("/dev/rbd3").await.unwrap();
            assert!(tools.calls("rbd")[0].starts_with("unmap /dev/rbd3 -m "));
        }
        let _tools = FakeTools::install(&[("rbd", "echo busy >&2\nexit 16")]);
        let error = unmap_device("/dev/rbd3").await.unwrap_err().to_string();
        assert!(error.contains("rbd unmap failed"));
    }

    #[tokio::test]
    async fn mapped_images_are_listed_from_showmapped() {
        {
            let tools = FakeTools::install(&[(
                "rbd",
                r#"echo '[{"pool":"csfx","name":"vol-1","device":"/dev/rbd0"}]'"#,
            )]);
            let mapped = list_mapped().await.unwrap();
            assert_eq!(mapped.len(), 1);
            assert_eq!(
                tools.calls("rbd"),
                vec!["showmapped --format json".to_string()]
            );
        }
        let _tools = FakeTools::install(&[("rbd", "echo down >&2\nexit 1")]);
        assert!(list_mapped()
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("rbd showmapped failed"));
    }

    #[tokio::test]
    async fn formatted_devices_are_left_alone() {
        let tools = FakeTools::install(&[("blkid", "exit 0"), ("mkfs.ext4", "exit 0")]);

        ensure_filesystem("/dev/rbd0", "data").await.unwrap();

        assert_eq!(tools.calls("blkid"), vec!["-p /dev/rbd0".to_string()]);
        assert!(tools.calls("mkfs.ext4").is_empty());
    }

    #[tokio::test]
    async fn images_smaller_than_requested_are_resized() {
        let tools = FakeTools::install(&[("blockdev", "echo 5368709120"), ("rbd", "exit 0")]);

        assert!(grow_image_to("csfx", "vol-1", "/dev/loop0", 10)
            .await
            .unwrap());
        assert!(tools.calls("rbd")[0].starts_with("resize csfx/vol-1 --size 10240"));
    }

    #[tokio::test]
    async fn images_at_the_requested_size_are_left_alone() {
        let tools = FakeTools::install(&[("blockdev", "echo 10737418240"), ("rbd", "exit 0")]);

        assert!(!grow_image_to("csfx", "vol-1", "/dev/loop0", 10)
            .await
            .unwrap());
        assert!(tools.calls("rbd").is_empty());
    }

    const SYSTEMD_RUN: &str = "while [ \"$1\" != \"--\" ]; do shift; done\nshift\nexec \"$@\"";

    #[tokio::test]
    async fn growing_checks_then_resizes_the_filesystem() {
        let tools = FakeTools::install(&[
            ("systemd-run", SYSTEMD_RUN),
            ("e2fsck", "exit 1"),
            ("resize2fs", "exit 0"),
        ]);

        grow_filesystem("/dev/rbd0").await.unwrap();

        assert_eq!(tools.calls("e2fsck"), vec!["-fp /dev/rbd0".to_string()]);
        assert_eq!(tools.calls("resize2fs"), vec!["/dev/rbd0".to_string()]);
    }

    #[tokio::test]
    async fn filesystem_tools_only_run_inside_a_sandboxed_transient_unit() {
        let tools = FakeTools::install(&[
            ("systemd-run", SYSTEMD_RUN),
            ("e2fsck", "exit 0"),
            ("resize2fs", "exit 0"),
        ]);

        grow_filesystem("/dev/rbd0").await.unwrap();

        let launches = tools.calls("systemd-run");
        assert_eq!(launches.len(), 2);
        for launch in launches {
            for property in [
                "DynamicUser=yes",
                "PrivateNetwork=yes",
                "SystemCallFilter=@system-service",
                "DevicePolicy=closed",
                "DeviceAllow=/dev/rbd0 rw",
                "CapabilityBoundingSet=",
            ] {
                assert!(launch.contains(property), "{property}: {launch}");
            }
            assert!(launch.contains(" -- e2fsck ") || launch.contains(" -- resize2fs "));
        }
    }

    #[tokio::test]
    async fn growing_stops_when_the_check_finds_uncorrectable_errors() {
        let tools = FakeTools::install(&[
            ("systemd-run", SYSTEMD_RUN),
            ("e2fsck", "exit 4"),
            ("resize2fs", "exit 0"),
        ]);

        assert!(grow_filesystem("/dev/rbd0").await.is_err());
        assert!(tools.calls("resize2fs").is_empty());
    }

    #[tokio::test]
    async fn empty_devices_are_formatted_with_a_label() {
        let tools = FakeTools::install(&[("blkid", "exit 2"), ("mkfs.ext4", "exit 0")]);

        ensure_filesystem("/dev/rbd0", "data").await.unwrap();

        assert_eq!(
            tools.calls("mkfs.ext4"),
            vec!["-q -L data /dev/rbd0".to_string()]
        );
    }

    #[tokio::test]
    async fn probe_and_format_failures_are_reported() {
        {
            let _tools =
                FakeTools::install(&[("blkid", "echo io >&2\nexit 4"), ("mkfs.ext4", "exit 0")]);
            let error = ensure_filesystem("/dev/rbd0", "data")
                .await
                .unwrap_err()
                .to_string();
            assert!(error.contains("blkid probe failed"));
        }
        let _tools =
            FakeTools::install(&[("blkid", "exit 2"), ("mkfs.ext4", "echo full >&2\nexit 1")]);
        let error = ensure_filesystem("/dev/rbd0", "data")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("mkfs.ext4 failed"));
    }
}
