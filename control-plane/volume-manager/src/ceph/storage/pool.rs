use crate::ceph::core::CephClient;
use super::types::*;
use anyhow::{Context, Result};

pub struct PoolManager {
    client: CephClient,
}

impl PoolManager {
    pub fn new(client: CephClient) -> Self {
        Self { client }
    }

    /// Erstellt einen neuen Ceph Pool
    pub async fn create_pool(&self, pool: &CephPool) -> Result<()> {
        crate::log_info!(
            "pool_manager",
            &format!("Creating Ceph pool: {}", pool.name)
        );

        // Pool erstellen
        let cmd = CephCommand::new("osd")
            .arg("pool")
            .arg("create")
            .arg(&pool.name)
            .arg(pool.pg_num.to_string())
            .arg(pool.pgp_num.to_string());

        self.client.execute(cmd).await
            .context("Failed to create pool")?;

        // Replikation setzen
        let cmd = CephCommand::new("osd")
            .arg("pool")
            .arg("set")
            .arg(&pool.name)
            .arg("size")
            .arg(pool.size.to_string());

        self.client.execute(cmd).await
            .context("Failed to set pool size")?;

        // Min size setzen
        let cmd = CephCommand::new("osd")
            .arg("pool")
            .arg("set")
            .arg(&pool.name)
            .arg("min_size")
            .arg(pool.min_size.to_string());

        self.client.execute(cmd).await
            .context("Failed to set pool min_size")?;

        // RBD Pool initialisieren
        let cmd = CephCommand::new("osd")
            .arg("pool")
            .arg("application")
            .arg("enable")
            .arg(&pool.name)
            .arg("rbd");

        self.client.execute(cmd).await
            .context("Failed to enable RBD application")?;

        crate::log_info!(
            "pool_manager",
            &format!("Pool '{}' created successfully", pool.name)
        );

        Ok(())
    }

    /// Löscht einen Pool
    pub async fn delete_pool(&self, pool_name: &str) -> Result<()> {
        crate::log_info!(
            "pool_manager",
            &format!("Deleting Ceph pool: {}", pool_name)
        );

        let cmd = CephCommand::new("osd")
            .arg("pool")
            .arg("delete")
            .arg(pool_name)
            .arg(pool_name) // Bestätigung
            .arg("--yes-i-really-really-mean-it");

        self.client.execute(cmd).await
            .context("Failed to delete pool")?;

        Ok(())
    }

    /// Listet alle Pools auf
    pub async fn list_pools(&self) -> Result<Vec<String>> {
        let cmd = CephCommand::new("osd")
            .arg("pool")
            .arg("ls");

        let output = self.client.execute(cmd).await?;
        let pools: Vec<String> = serde_json::from_str(&output)?;
        Ok(pools)
    }

    /// Prüft ob Pool existiert
    pub async fn pool_exists(&self, pool_name: &str) -> Result<bool> {
        let pools = self.list_pools().await?;
        Ok(pools.contains(&pool_name.to_string()))
    }

    /// Erstellt Pool falls nicht vorhanden
    pub async fn ensure_pool(&self, pool: &CephPool) -> Result<()> {
        if !self.pool_exists(&pool.name).await? {
            self.create_pool(pool).await?;
        } else {
            crate::log_info!(
                "pool_manager",
                &format!("Pool '{}' already exists", pool.name)
            );
        }
        Ok(())
    }
}
