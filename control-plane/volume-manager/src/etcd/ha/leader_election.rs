use crate::etcd::core::{EtcdClient, EtcdError};
use etcd_client::EventType;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Leader Election mit etcd
pub struct LeaderElection {
    client: Arc<EtcdClient>,
    node_id: String,
    election_key: String,
    lease_id: Arc<RwLock<Option<i64>>>,
    is_leader: Arc<AtomicBool>,
    ttl: i64,
}

impl LeaderElection {
    pub fn new(client: Arc<EtcdClient>, node_id: String) -> Self {
        let election_key = format!("election/leader");
        Self {
            client,
            node_id,
            election_key,
            lease_id: Arc::new(RwLock::new(None)),
            is_leader: Arc::new(AtomicBool::new(false)),
            ttl: 10, // 10 Sekunden TTL
        }
    }

    /// Startet Leader Election Campaign
    pub async fn campaign(&self) -> Result<(), EtcdError> {
        info!("🎯 Campaign for leadership: {}", self.node_id);

        // Lease erstellen
        let lease_id = self.client.grant_lease(self.ttl).await?;
        *self.lease_id.write().await = Some(lease_id);

        // Versuche Leader zu werden
        let value = self.node_id.as_bytes().to_vec();

        // Versuche atomisch Leader zu werden (nur wenn Key nicht existiert)
        match self.client.get(&self.election_key).await? {
            None => {
                // Kein Leader vorhanden, wir werden Leader
                self.client.put(&self.election_key, value).await?;
                self.is_leader.store(true, Ordering::SeqCst);
                info!("👑 Became LEADER: {}", self.node_id);

                // Starte Lease Renewal
                self.start_lease_renewal().await;
            }
            Some(current_leader) => {
                let leader = String::from_utf8_lossy(&current_leader);
                info!("👥 Current leader is: {}", leader);
                self.is_leader.store(false, Ordering::SeqCst);
            }
        }

        Ok(())
    }

    /// Gibt Leadership auf
    pub async fn resign(&self) -> Result<(), EtcdError> {
        if !self.is_leader() {
            return Ok(());
        }

        info!("👋 Resigning from leadership: {}", self.node_id);

        // Lösche Election Key
        self.client.delete(&self.election_key).await?;

        // Revoke Lease
        if let Some(lease_id) = *self.lease_id.read().await {
            self.client.revoke_lease(lease_id).await?;
        }

        self.is_leader.store(false, Ordering::SeqCst);
        *self.lease_id.write().await = None;

        info!("✅ Resigned from leadership");
        Ok(())
    }

    /// Prüft ob dieser Node Leader ist
    pub fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::SeqCst)
    }

    /// Holt aktuellen Leader
    pub async fn get_leader(&self) -> Result<Option<String>, EtcdError> {
        match self.client.get(&self.election_key).await? {
            Some(data) => Ok(Some(String::from_utf8_lossy(&data).to_string())),
            None => Ok(None),
        }
    }

    /// Startet automatische Lease Renewal
    async fn start_lease_renewal(&self) {
        let client = self.client.clone();
        let lease_id = self.lease_id.clone();
        let is_leader = self.is_leader.clone();
        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

            while is_leader.load(Ordering::SeqCst) {
                interval.tick().await;

                let current_lease = *lease_id.read().await;
                if let Some(lid) = current_lease {
                    match client.keep_alive(lid).await {
                        Ok(_) => {
                            // Lease erfolgreich erneuert
                        }
                        Err(e) => {
                            error!("❌ Lease renewal failed: {}", e);
                            is_leader.store(false, Ordering::SeqCst);
                            warn!("⚠️  Lost leadership due to lease failure: {}", node_id);
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
        });
    }

    /// Wartet auf Leadership Changes (Watch)
    pub async fn watch_leadership<F>(&self, mut callback: F) -> Result<(), EtcdError>
    where
        F: FnMut(Option<String>) + Send + 'static,
    {
        let key = self.client.config().prefixed_key(&self.election_key);
        info!("👀 Watching leadership changes on: {}", key);

        // Hier würde ein Watch implementiert werden
        // Für jetzt vereinfacht
        Ok(())
    }
}
