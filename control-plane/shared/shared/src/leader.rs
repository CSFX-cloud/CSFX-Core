use anyhow::{bail, Context, Result};
use etcd_client::{Client, Compare, CompareOp, PutOptions, Txn, TxnOp};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout, Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

const DEFAULT_ETCD_ENDPOINTS: &str = "http://localhost:2379";
const LEASE_TTL_SECONDS: i64 = 10;
const RENEW_INTERVAL: Duration = Duration::from_secs(2);
const RENEW_TIMEOUT: Duration = Duration::from_secs(2);
const LEADER_VALIDITY: Duration = Duration::from_secs(6);
const CAMPAIGN_INTERVAL: Duration = Duration::from_secs(5);

const _: () = assert!(LEADER_VALIDITY.as_secs() < LEASE_TTL_SECONDS as u64);
const _: () =
    assert!(RENEW_INTERVAL.as_secs() + RENEW_TIMEOUT.as_secs() < LEADER_VALIDITY.as_secs());

#[derive(Clone)]
struct LeaderClock {
    origin: Instant,
    valid_until_ms: Arc<AtomicU64>,
}

impl LeaderClock {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            valid_until_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    fn extend_from(&self, renewal_started: Instant) {
        let valid_until = renewal_started + LEADER_VALIDITY - self.origin;
        self.valid_until_ms
            .store(valid_until.as_millis() as u64, Ordering::SeqCst);
    }

    fn revoke(&self) {
        self.valid_until_ms.store(0, Ordering::SeqCst);
    }

    fn is_valid(&self) -> bool {
        let valid_until = self.valid_until_ms.load(Ordering::SeqCst);
        valid_until != 0 && (self.origin.elapsed().as_millis() as u64) < valid_until
    }
}

#[derive(Clone)]
pub struct LeaderLease {
    etcd: Client,
    key: String,
    node_id: String,
    clock: LeaderClock,
}

impl LeaderLease {
    pub fn new(etcd: Client, key: impl Into<String>, node_id: impl Into<String>) -> Self {
        Self {
            etcd,
            key: key.into(),
            node_id: node_id.into(),
            clock: LeaderClock::new(),
        }
    }

    pub async fn connect(key: &str) -> Result<Self> {
        let endpoints =
            std::env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| DEFAULT_ETCD_ENDPOINTS.to_string());
        let etcd = Client::connect(endpoints.split(',').collect::<Vec<_>>(), None)
            .await
            .context("failed to connect to etcd")?;
        Ok(Self::new(etcd, key, Uuid::new_v4().to_string()).spawn())
    }

    pub fn is_leader(&self) -> bool {
        self.clock.is_valid()
    }

    pub fn spawn(self) -> Self {
        tokio::spawn(self.clone().run());
        self
    }

    async fn run(mut self) {
        loop {
            let started = Instant::now();
            match self.campaign().await {
                Ok(Some(lease_id)) => self.lead(lease_id, started).await,
                Ok(None) => {}
                Err(e) => error!(key = %self.key, error = %e, "leader campaign failed"),
            }
            sleep(CAMPAIGN_INTERVAL).await;
        }
    }

    async fn lead(&mut self, lease_id: i64, acquired_at: Instant) {
        self.clock.extend_from(acquired_at);
        info!(key = %self.key, node_id = %self.node_id, "leadership acquired");
        if let Err(e) = self.hold(lease_id).await {
            warn!(key = %self.key, node_id = %self.node_id, error = %e, "leadership lost");
        }
        self.clock.revoke();
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
        if self
            .etcd
            .txn(txn)
            .await
            .context("etcd txn failed")?
            .succeeded()
        {
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
            let renewal_started = Instant::now();
            let renewed = timeout(RENEW_TIMEOUT, async {
                keeper.keep_alive().await?;
                stream.message().await
            })
            .await
            .context("lease renewal timed out")??;
            match renewed {
                Some(response) if response.ttl() > 0 => self.clock.extend_from(renewal_started),
                _ => bail!("lease expired"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn leadership_lapses_before_lease_ttl_when_renewal_hangs() {
        let clock = LeaderClock::new();
        assert!(!clock.is_valid());
        clock.extend_from(Instant::now());
        assert!(clock.is_valid());

        sleep(RENEW_INTERVAL).await;
        clock.extend_from(Instant::now());
        sleep(LEADER_VALIDITY - Duration::from_millis(1)).await;
        assert!(clock.is_valid());

        sleep(Duration::from_millis(1)).await;
        assert!(!clock.is_valid());
    }

    #[tokio::test(start_paused = true)]
    async fn revoke_clears_leadership_immediately() {
        let clock = LeaderClock::new();
        clock.extend_from(Instant::now());
        clock.revoke();
        assert!(!clock.is_valid());
    }
}
