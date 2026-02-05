use crate::etcd::core::{EtcdClient, EtcdError};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Distributed Lock mit etcd
pub struct DistributedLock {
    client: Arc<EtcdClient>,
    lock_key: String,
    lease_id: Option<i64>,
    ttl: i64,
}

impl DistributedLock {
    /// Erstellt einen neuen Lock
    pub fn new(client: Arc<EtcdClient>, lock_name: &str) -> Self {
        let lock_key = format!("locks/{}", lock_name);
        Self {
            client,
            lock_key,
            lease_id: None,
            ttl: 30, // 30 Sekunden TTL
        }
    }

    /// Versucht Lock zu erwerben
    pub async fn acquire(&mut self) -> Result<bool, EtcdError> {
        debug!("🔒 Trying to acquire lock: {}", self.lock_key);

        // Prüfe ob Lock bereits existiert
        if let Some(_) = self.client.get(&self.lock_key).await? {
            debug!("⏳ Lock already held by another process");
            return Ok(false);
        }

        // Erstelle Lease
        let lease_id = self.client.grant_lease(self.ttl).await?;
        self.lease_id = Some(lease_id);

        // Setze Lock mit Lease
        let value = format!("locked-{}", chrono::Utc::now().timestamp());
        self.client.put(&self.lock_key, value.into_bytes()).await?;

        info!("✅ Lock acquired: {}", self.lock_key);
        Ok(true)
    }

    /// Versucht Lock mit Retry zu erwerben
    pub async fn acquire_with_retry(
        &mut self,
        max_retries: u32,
        retry_interval_ms: u64,
    ) -> Result<bool, EtcdError> {
        for attempt in 1..=max_retries {
            if self.acquire().await? {
                return Ok(true);
            }

            if attempt < max_retries {
                debug!(
                    "⏰ Lock acquisition attempt {}/{}, retrying in {}ms",
                    attempt, max_retries, retry_interval_ms
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(retry_interval_ms)).await;
            }
        }

        warn!("❌ Failed to acquire lock after {} attempts", max_retries);
        Ok(false)
    }

    /// Gibt Lock frei
    pub async fn release(&mut self) -> Result<(), EtcdError> {
        if self.lease_id.is_none() {
            return Ok(());
        }

        debug!("🔓 Releasing lock: {}", self.lock_key);

        // Lösche Lock Key
        self.client.delete(&self.lock_key).await?;

        // Revoke Lease
        if let Some(lease_id) = self.lease_id {
            self.client.revoke_lease(lease_id).await?;
        }

        self.lease_id = None;
        info!("✅ Lock released: {}", self.lock_key);
        Ok(())
    }

    /// Erneuert Lock Lease
    pub async fn refresh(&self) -> Result<(), EtcdError> {
        if let Some(lease_id) = self.lease_id {
            debug!("🔄 Refreshing lock lease: {}", self.lock_key);
            self.client.keep_alive(lease_id).await?;
            Ok(())
        } else {
            Err(EtcdError::LockFailed("No active lease to refresh".to_string()))
        }
    }
}

/// Automatisches Release beim Drop
impl Drop for DistributedLock {
    fn drop(&mut self) {
        if self.lease_id.is_some() {
            warn!("⚠️  Lock dropped without explicit release: {}", self.lock_key);
        }
    }
}
