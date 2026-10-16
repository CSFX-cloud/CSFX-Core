use anyhow::{bail, Context, Result};
use etcd_client::{Client, Compare, CompareOp, PutOptions, Txn, TxnOp};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn};

const LEASE_TTL_SECONDS: i64 = 10;
const RENEW_INTERVAL: Duration = Duration::from_secs(3);
const RENEW_TIMEOUT: Duration = Duration::from_secs(LEASE_TTL_SECONDS as u64);
const CAMPAIGN_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct LeaderLease {
    etcd: Client,
    key: String,
    node_id: String,
    is_leader: Arc<AtomicBool>,
}

impl LeaderLease {
    pub fn new(etcd: Client, key: impl Into<String>, node_id: impl Into<String>) -> Self {
        Self {
            etcd,
            key: key.into(),
            node_id: node_id.into(),
            is_leader: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::SeqCst)
    }

    pub fn spawn(self) -> Self {
        tokio::spawn(self.clone().run());
        self
    }

    async fn run(mut self) {
        loop {
            match self.campaign().await {
                Ok(Some(lease_id)) => self.lead(lease_id).await,
                Ok(None) => {}
                Err(e) => error!(key = %self.key, error = %e, "leader campaign failed"),
            }
            sleep(CAMPAIGN_INTERVAL).await;
        }
    }

    async fn lead(&mut self, lease_id: i64) {
        self.is_leader.store(true, Ordering::SeqCst);
        info!(key = %self.key, node_id = %self.node_id, "leadership acquired");
        if let Err(e) = self.hold(lease_id).await {
            warn!(key = %self.key, node_id = %self.node_id, error = %e, "leadership lost");
        }
        self.is_leader.store(false, Ordering::SeqCst);
    }

    async fn campaign(&mut self) -> Result<Option<i64>> {
        let lease_id = self
            .etcd
            .lease_grant(LEASE_TTL_SECONDS, None)
            .await
            .context("failed to grant etcd lease")?
            .id();
        let put = TxnOp::put(
            self.key.as_str(),
            self.node_id.as_bytes(),
            Some(PutOptions::new().with_lease(lease_id)),
        );
        let txn = Txn::new()
            .when(vec![Compare::create_revision(
                self.key.as_str(),
                CompareOp::Equal,
                0,
            )])
            .and_then(vec![put]);
        if self.etcd.txn(txn).await.context("etcd txn failed")?.succeeded() {
            return Ok(Some(lease_id));
        }
        self.etcd
            .lease_revoke(lease_id)
            .await
            .context("failed to revoke unused lease")?;
        Ok(None)
    }

    async fn hold(&mut self, lease_id: i64) -> Result<()> {
        let (mut keeper, mut stream) = self
            .etcd
            .lease_keep_alive(lease_id)
            .await
            .context("failed to open keep-alive stream")?;
        loop {
            sleep(RENEW_INTERVAL).await;
            let renewed = timeout(RENEW_TIMEOUT, async {
                keeper.keep_alive().await?;
                stream.message().await
            })
            .await
            .context("lease renewal timed out")??;
            match renewed {
                Some(response) if response.ttl() > 0 => {}
                _ => bail!("lease expired"),
            }
        }
    }
}
