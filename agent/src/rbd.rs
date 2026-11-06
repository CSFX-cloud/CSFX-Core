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

#[cfg(test)]
mod tests {
    use super::parse_showmapped;

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
}
