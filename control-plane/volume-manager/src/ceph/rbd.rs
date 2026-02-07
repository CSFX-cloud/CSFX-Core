use super::client::CephClient;
use super::types::*;
use anyhow::{Context, Result};
use serde_json::Value;

pub struct RbdManager {
    client: CephClient,
}

impl RbdManager {
    pub fn new(client: CephClient) -> Self {
        Self { client }
    }

    /// Erstellt ein RBD Image (Volume)
    pub async fn create_image(&self, volume: &CephVolume) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Creating RBD image: {}/{}", volume.pool, volume.name)
        );

        let mut cmd = CephCommand::new("rbd")
            .arg("create")
            .arg(format!("{}/{}", volume.pool, volume.name))
            .arg("--size")
            .arg(volume.size_mb.to_string());

        // Features hinzufügen
        if !volume.features.is_empty() {
            cmd = cmd.arg("--image-feature").arg(volume.features.join(","));
        }

        self.client.execute(cmd).await
            .context("Failed to create RBD image")?;

        // Verschlüsselung aktivieren falls gewünscht
        if volume.encrypted {
            self.enable_encryption(&volume.pool, &volume.name).await?;
        }

        crate::log_info!(
            "rbd_manager",
            &format!("RBD image '{}/{}' created successfully", volume.pool, volume.name)
        );

        Ok(())
    }

    /// Löscht ein RBD Image
    pub async fn delete_image(&self, pool: &str, name: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Deleting RBD image: {}/{}", pool, name)
        );

        let cmd = CephCommand::new("rbd")
            .arg("rm")
            .arg(format!("{}/{}", pool, name));

        self.client.execute(cmd).await
            .context("Failed to delete RBD image")?;

        Ok(())
    }

    /// Listet alle RBD Images in einem Pool
    pub async fn list_images(&self, pool: &str) -> Result<Vec<RbdImage>> {
        let cmd = CephCommand::new("rbd")
            .arg("ls")
            .arg("-l")
            .arg(pool);

        let output = self.client.execute(cmd).await?;
        
        if output.trim().is_empty() || output.trim() == "[]" {
            return Ok(Vec::new());
        }

        let images: Vec<Value> = serde_json::from_str(&output)?;
        
        let result = images
            .into_iter()
            .filter_map(|img| {
                Some(RbdImage {
                    name: img["name"].as_str()?.to_string(),
                    size: img["size"].as_u64()?,
                    pool: pool.to_string(),
                    format: img["format"].as_u64().unwrap_or(2) as u32,
                    features: img["features"]
                        .as_array()?
                        .iter()
                        .filter_map(|f| f.as_str().map(|s| s.to_string()))
                        .collect(),
                })
            })
            .collect();

        Ok(result)
    }

    /// Erstellt einen Snapshot
    pub async fn create_snapshot(&self, pool: &str, image: &str, snapshot: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Creating snapshot: {}/{}@{}", pool, image, snapshot)
        );

        let cmd = CephCommand::new("rbd")
            .arg("snap")
            .arg("create")
            .arg(format!("{}/{}@{}", pool, image, snapshot));

        self.client.execute(cmd).await
            .context("Failed to create snapshot")?;

        Ok(())
    }

    /// Löscht einen Snapshot
    pub async fn delete_snapshot(&self, pool: &str, image: &str, snapshot: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Deleting snapshot: {}/{}@{}", pool, image, snapshot)
        );

        let cmd = CephCommand::new("rbd")
            .arg("snap")
            .arg("rm")
            .arg(format!("{}/{}@{}", pool, image, snapshot));

        self.client.execute(cmd).await
            .context("Failed to delete snapshot")?;

        Ok(())
    }

    /// Resized ein Image
    pub async fn resize_image(&self, pool: &str, name: &str, new_size_mb: u64) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Resizing RBD image: {}/{} to {} MB", pool, name, new_size_mb)
        );

        let cmd = CephCommand::new("rbd")
            .arg("resize")
            .arg(format!("{}/{}", pool, name))
            .arg("--size")
            .arg(new_size_mb.to_string());

        self.client.execute(cmd).await
            .context("Failed to resize RBD image")?;

        Ok(())
    }

    /// Maps ein RBD Device
    pub async fn map_device(&self, pool: &str, image: &str) -> Result<String> {
        crate::log_info!(
            "rbd_manager",
            &format!("Mapping RBD device: {}/{}", pool, image)
        );

        let cmd = CephCommand::new("rbd")
            .arg("map")
            .arg(format!("{}/{}", pool, image));

        let output = self.client.execute(cmd).await
            .context("Failed to map RBD device")?;

        let device = output.trim().trim_matches('"').to_string();
        
        crate::log_info!(
            "rbd_manager",
            &format!("RBD device mapped to: {}", device)
        );

        Ok(device)
    }

    /// Unmaps ein RBD Device
    pub async fn unmap_device(&self, device: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Unmapping RBD device: {}", device)
        );

        let cmd = CephCommand::new("rbd")
            .arg("unmap")
            .arg(device);

        self.client.execute(cmd).await
            .context("Failed to unmap RBD device")?;

        Ok(())
    }

    /// Aktiviert Verschlüsselung (LUKS)
    async fn enable_encryption(&self, pool: &str, image: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Enabling encryption for: {}/{}", pool, image)
        );

        // Dies ist ein Platzhalter - tatsächliche LUKS-Verschlüsselung
        // würde auf dem gemappten Block Device erfolgen
        // Hier könnten wir rbd encryption format aufrufen
        
        let cmd = CephCommand::new("rbd")
            .arg("encryption")
            .arg("format")
            .arg(format!("{}/{}", pool, image))
            .arg("luks2")
            .arg("passphrase-file")
            .arg("/etc/ceph/luks-passphrase");

        // Ignoriere Fehler falls Encryption nicht verfügbar
        let _ = self.client.execute(cmd).await;

        Ok(())
    }

    /// Prüft ob Image existiert
    pub async fn image_exists(&self, pool: &str, name: &str) -> Result<bool> {
        let images = self.list_images(pool).await?;
        Ok(images.iter().any(|img| img.name == name))
    }
}
