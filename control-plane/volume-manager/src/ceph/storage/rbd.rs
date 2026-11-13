use super::types::*;
use crate::ceph::core::error::CephError;
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
        match self.client.execute_rbd(cmd).await {
            Ok(_) => Ok(true),
            Err(error) if is_not_found(&error) => Ok(false),
            Err(error) => Err(error.context("Failed to probe RBD image")),
        }
    }

    pub async fn list_images(&self, pool: &str) -> Result<Vec<String>> {
        let cmd = CephCommand::new("ls").arg(pool).arg("--format").arg("json");
        let output = self
            .client
            .execute_rbd(cmd)
            .await
            .context("Failed to list RBD images")?;
        serde_json::from_str(&output).context("Invalid RBD image list output")
    }

    pub async fn purge_snapshots(&self, pool: &str, image: &str) -> Result<()> {
        crate::log_info!(
            "rbd_manager",
            &format!("Purging snapshots: {}/{}", pool, image)
        );

        let cmd = CephCommand::new("snap")
            .arg("purge")
            .arg(format!("{}/{}", pool, image));

        self.client
            .execute_rbd(cmd)
            .await
            .context("Failed to purge snapshots")?;

        Ok(())
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

const NOT_FOUND_EXIT_CODE: i32 = 2;
const NOT_FOUND_MARKERS: [&str; 2] = ["No such file or directory", "ENOENT"];

fn classify_not_found(code: Option<i32>, stderr: &str) -> bool {
    code == Some(NOT_FOUND_EXIT_CODE)
        || NOT_FOUND_MARKERS
            .iter()
            .any(|marker| stderr.contains(marker))
}

pub fn is_not_found(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<CephError>(),
        Some(CephError::CommandExit { code, stderr, .. }) if classify_not_found(*code, stderr)
    )
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
    use super::{classify_not_found, parse_watcher_addresses};

    #[test]
    fn classifies_not_found() {
        assert!(classify_not_found(Some(2), ""));
        assert!(classify_not_found(
            Some(1),
            "rbd: error opening image x: (2) No such file or directory"
        ));
        assert!(classify_not_found(None, "error ENOENT"));
    }

    #[test]
    fn other_failures_are_not_not_found() {
        assert!(!classify_not_found(Some(1), "unable to connect to cluster"));
        assert!(!classify_not_found(Some(110), "Connection timed out"));
        assert!(!classify_not_found(None, ""));
    }

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
