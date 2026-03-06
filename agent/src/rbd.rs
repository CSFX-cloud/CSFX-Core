use anyhow::{Context, Result, anyhow};
use tokio::process::Command;
use tracing::{info, warn};

pub async fn map_device(pool: &str, image: &str) -> Result<String> {
    info!(pool = %pool, image = %image, "Mapping RBD device");

    let output = Command::new("rbd")
        .args(["map", &format!("{}/{}", pool, image)])
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

    let output = Command::new("rbd")
        .args(["unmap", device])
        .output()
        .await
        .context("Failed to execute rbd unmap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(device = %device, error = %stderr, "rbd unmap failed");
        return Err(anyhow!("rbd unmap failed: {}", stderr));
    }

    info!(device = %device, "RBD device unmapped");
    Ok(())
}

pub async fn mount(device: &str, mount_point: &str) -> Result<()> {
    info!(device = %device, mount_point = %mount_point, "Mounting RBD device");

    tokio::fs::create_dir_all(mount_point)
        .await
        .context("Failed to create mount point")?;

    let output = Command::new("mount")
        .args([device, mount_point])
        .output()
        .await
        .context("Failed to execute mount")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("mount failed: {}", stderr));
    }

    info!(device = %device, mount_point = %mount_point, "Device mounted");
    Ok(())
}

pub async fn umount(mount_point: &str) -> Result<()> {
    info!(mount_point = %mount_point, "Unmounting device");

    let output = Command::new("umount")
        .arg(mount_point)
        .output()
        .await
        .context("Failed to execute umount")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(mount_point = %mount_point, error = %stderr, "umount failed");
        return Err(anyhow!("umount failed: {}", stderr));
    }

    info!(mount_point = %mount_point, "Device unmounted");
    Ok(())
}

pub fn mount_point_for(volume_id: &str) -> String {
    format!("/mnt/csf-volumes/{}", volume_id)
}
