use anyhow::Result;
use etcd_client::{Compare, CompareOp, GetOptions, PutOptions, Txn, TxnOp};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::error;

use crate::config::Config;

pub const DESIRED_VERSION_KEY: &str = "/csfx/config/desired_version";
pub const BUILD_STATUS_KEY: &str = "/csfx/config/cp_build_status";
pub const RESULT_KEY: &str = "/csfx/config/last_build_result";
pub const PAUSED_KEY: &str = "/csfx/config/update_paused";
pub const FORCE_UPDATE_KEY: &str = "/csfx/config/update_force";
pub const MANIFEST_PREFIX: &str = "/csfx/config/release_manifest/";
pub const MANIFEST_SIGNATURE_PREFIX: &str = "/csfx/config/release_manifest_sig/";
pub const WORKER_MANIFEST_PREFIX: &str = "/csfx/config/worker_release_manifest/";
pub const WORKER_MANIFEST_SIGNATURE_PREFIX: &str = "/csfx/config/worker_release_manifest_sig/";
pub const APPLIED_PREFIX: &str = "/csfx/updater/applied/";
pub const UPDATE_LOCK_KEY: &str = "/csfx/updater/lock";

pub struct Client {
    inner: etcd_client::Client,
}

pub struct Lock {
    client: etcd_client::Client,
    lease_id: i64,
    keepalive: JoinHandle<()>,
}

impl Lock {
    pub async fn release(mut self) -> Result<()> {
        self.keepalive.abort();
        self.client.lease_revoke(self.lease_id).await?;
        Ok(())
    }
}

impl Client {
    pub async fn connect(cfg: &Config) -> Result<Self> {
        let endpoints: Vec<&str> = cfg.etcd_endpoints.iter().map(|s| s.as_str()).collect();
        let inner = etcd_client::Client::connect(endpoints, None).await?;
        Ok(Self { inner })
    }

    pub async fn get(&mut self, key: &str) -> Result<Option<String>> {
        let resp = self.inner.get(key, None).await?;
        Ok(resp
            .kvs()
            .first()
            .and_then(|kv| std::str::from_utf8(kv.value()).ok())
            .map(|s| s.to_string()))
    }

    pub async fn put(&mut self, key: &str, value: &str) -> Result<()> {
        self.inner.put(key, value.as_bytes(), None).await?;
        Ok(())
    }

    pub async fn delete(&mut self, key: &str) -> Result<()> {
        self.inner.delete(key, None).await?;
        Ok(())
    }

    pub async fn values_with_prefix(&mut self, prefix: &str) -> Result<Vec<String>> {
        let resp = self
            .inner
            .get(prefix, Some(GetOptions::new().with_prefix()))
            .await?;
        Ok(resp
            .kvs()
            .iter()
            .filter_map(|kv| std::str::from_utf8(kv.value()).ok())
            .map(str::to_string)
            .collect())
    }

    pub async fn member_count(&mut self) -> Result<usize> {
        Ok(self.inner.member_list().await?.members().len())
    }

    pub async fn acquire_lock(
        &mut self,
        key: &str,
        holder: &str,
        ttl_secs: i64,
    ) -> Result<Option<Lock>> {
        if let Some(lock) = self.try_lock(key, holder, ttl_secs).await? {
            return Ok(Some(lock));
        }
        if self.get(key).await?.as_deref() != Some(holder) {
            return Ok(None);
        }
        self.delete(key).await?;
        self.try_lock(key, holder, ttl_secs).await
    }

    async fn try_lock(&mut self, key: &str, holder: &str, ttl_secs: i64) -> Result<Option<Lock>> {
        let lease_id = self.inner.lease_grant(ttl_secs, None).await?.id();
        let claim = Txn::new()
            .when([Compare::create_revision(key, CompareOp::Equal, 0)])
            .and_then([TxnOp::put(
                key,
                holder,
                Some(PutOptions::new().with_lease(lease_id)),
            )]);
        if !self.inner.txn(claim).await?.succeeded() {
            self.inner.lease_revoke(lease_id).await?;
            return Ok(None);
        }
        let keepalive = self.spawn_keepalive(lease_id, ttl_secs).await?;
        Ok(Some(Lock {
            client: self.inner.clone(),
            lease_id,
            keepalive,
        }))
    }

    async fn spawn_keepalive(&mut self, lease_id: i64, ttl_secs: i64) -> Result<JoinHandle<()>> {
        let (mut keeper, mut responses) = self.inner.lease_keep_alive(lease_id).await?;
        let interval = Duration::from_secs((ttl_secs / 3).max(1) as u64);
        Ok(tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let renewed =
                    keeper.keep_alive().await.is_ok() && responses.message().await.is_ok();
                if !renewed {
                    error!(lease_id, "update lock keepalive failed");
                    return;
                }
            }
        }))
    }
}
