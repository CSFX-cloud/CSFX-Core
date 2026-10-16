use super::types::*;
use crate::ceph::core::CephClient;
use anyhow::{Context, Result};
use serde_json::Value;

pub struct RbdManager {
    client: CephClient,
}

impl RbdManager {
    pub fn new(client: CephClient) -> Self {
        Self { client }
    }

    pub async fn create_image(&self, volume: &CephVolume) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Creating RBD image: {}/{}", volume.pool, volume.name)
        );

        let mut cmd = CephCommand::new("create")
            .arg(format!("{}/{}", volume.pool, volume.name))
            .arg("--size")
            .arg(volume.size_mb.to_string());

        if !volume.features.is_empty() {
            cmd = cmd.arg("--image-feature").arg(volume.features.join(","));
        }

        self.client
            .execute_rbd(cmd)
            .await
            .context("Failed to create RBD image")?;

        Ok(())
    }

    pub async fn delete_image(&self, pool: &str, name: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Deleting RBD image: {}/{}", pool, name)
        );

        let cmd = CephCommand::new("rm").arg(format!("{}/{}", pool, name));

        self.client
            .execute_rbd(cmd)
            .await
            .context("Failed to delete RBD image")?;

        Ok(())
    }

    pub async fn create_snapshot(&self, pool: &str, image: &str, snapshot: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Creating snapshot: {}/{}@{}", pool, image, snapshot)
        );

        let cmd = CephCommand::new("snap")
            .arg("create")
            .arg(format!("{}/{}@{}", pool, image, snapshot));

        self.client
            .execute_rbd(cmd)
            .await
            .context("Failed to create snapshot")?;

        Ok(())
    }

    pub async fn delete_snapshot(&self, pool: &str, image: &str, snapshot: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Deleting snapshot: {}/{}@{}", pool, image, snapshot)
        );

        let cmd = CephCommand::new("snap")
            .arg("rm")
            .arg(format!("{}/{}@{}", pool, image, snapshot));

        self.client
            .execute_rbd(cmd)
            .await
            .context("Failed to delete snapshot")?;

        Ok(())
    }

    pub async fn resize_image(&self, pool: &str, name: &str, new_size_mb: u64) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!(
                "Resizing RBD image: {}/{} to {} MB",
                pool, name, new_size_mb
            )
        );

        let cmd = CephCommand::new("resize")
            .arg(format!("{}/{}", pool, name))
            .arg("--size")
            .arg(new_size_mb.to_string());

        self.client
            .execute_rbd(cmd)
            .await
            .context("Failed to resize RBD image")?;

        Ok(())
    }

    pub async fn image_exists(&self, pool: &str, name: &str) -> Result<bool> {
        let cmd = CephCommand::new("info").arg(format!("{}/{}", pool, name));
        Ok(self.client.execute_rbd(cmd).await.is_ok())
    }

    pub async fn watcher_addresses(&self, pool: &str, image: &str) -> Result<Vec<String>> {
        let cmd = CephCommand::new("status")
            .arg(format!("{}/{}", pool, image))
            .arg("--format")
            .arg("json");

        let output = self
            .client
            .execute_rbd(cmd)
            .await
            .context("Failed to read RBD image status")?;

        parse_watcher_addresses(&output)
    }
}

fn parse_watcher_addresses(status_json: &str) -> Result<Vec<String>> {
    let status: Value =
        serde_json::from_str(status_json).context("Invalid RBD image status output")?;

    Ok(status["watchers"]
        .as_array()
        .map(|watchers| {
            watchers
                .iter()
                .filter_map(|watcher| watcher["address"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::parse_watcher_addresses;

    #[test]
    fn extracts_watcher_addresses() {
        let output =
            r#"{"watchers":[{"watcher":"client.4123","address":"10.0.0.5:0/3141592","cookie":1}]}"#;
        assert_eq!(
            parse_watcher_addresses(output).unwrap(),
            vec!["10.0.0.5:0/3141592".to_string()]
        );
    }

    #[test]
    fn missing_watchers_is_empty() {
        assert!(parse_watcher_addresses(r#"{"watchers":[]}"#)
            .unwrap()
            .is_empty());
        assert!(parse_watcher_addresses("{}").unwrap().is_empty());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_watcher_addresses("not json").is_err());
    }
}
