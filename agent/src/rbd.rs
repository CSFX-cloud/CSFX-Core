use anyhow::{anyhow, Context, Result};
use tokio::process::Command;
use tracing::info;

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
        .arg("exclusive");
    ceph_args.apply(&mut command);

    let output = command
        .output()
        .await
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
    command.arg("unmap").arg(device);
    ceph_args.apply(&mut command);

    let output = command
        .output()
        .await
        .context("Failed to execute rbd unmap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("rbd unmap failed: {}", stderr));
    }

    Ok(())
}
