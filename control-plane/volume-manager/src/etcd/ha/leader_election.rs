use crate::etcd::core::{EtcdClient, EtcdError};
use crate::{log_error, log_info, log_warn};
use etcd_client::EventType;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

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
        // Bin ich bereits Leader?
        if self.is_leader() {
            return Ok(()); // Bereits Leader, nichts zu tun
        }

        log_info!(
            "etcd::ha::leader_election",
            &format!("Attempting to become leader: {}", self.node_id)
        );

        // Prüfe ob bereits Leader aktiv ist
        match self.client.get_with_lease(&self.election_key).await? {
            Some((current_leader_bytes, lease_id)) => {
                let current_leader = String::from_utf8_lossy(&current_leader_bytes);

                // Prüfe ob der Lease noch gültig ist
                if lease_id > 0 {
                    match self.client.lease_time_to_live(lease_id).await? {
                        Some(ttl) => {
                            // Lease ist noch gültig, respektiere den aktuellen Leader
                            log_info!(
                                "etcd::ha::leader_election",
                                &format!(
                                    "Leader already exists: {} (Lease: {}, TTL: {}s)",
                                    current_leader, lease_id, ttl
                                )
                            );
                            return Ok(());
                        }
                        None => {
                            // Lease ist abgelaufen oder existiert nicht mehr!
                            log_warn!(
                                "etcd::ha::leader_election",
                                &format!(
                                    "Old leader {} has expired lease {}, taking over leadership",
                                    current_leader, lease_id
                                )
                            );
                            // Lösche den alten Key
                            if let Err(e) = self.client.delete(&self.election_key).await {
                                log_warn!(
                                    "etcd::ha::leader_election",
                                    &format!("Failed to delete old leader key: {}", e)
                                );
                            }
                        }
                    }
                } else {
                    // Kein Lease, der alte Eintrag ist ungültig
                    log_warn!(
                        "etcd::ha::leader_election",
                        &format!(
                            "Old leader {} found without valid lease, taking over leadership",
                            current_leader
                        )
                    );
                    // Lösche den alten Key
                    if let Err(e) = self.client.delete(&self.election_key).await {
                        log_warn!(
                            "etcd::ha::leader_election",
                            &format!("Failed to delete old leader key: {}", e)
                        );
                    }
                }
            }
            None => {
                // Kein Leader vorhanden
                log_info!(
                    "etcd::ha::leader_election",
                    &format!("No leader found, starting campaign: {}", self.node_id)
                );
            }
        }

        // Lease erstellen
        let lease_id = self.client.grant_lease(self.ttl).await?;
        *self.lease_id.write().await = Some(lease_id);

        // Versuche Leader zu werden mit atomarer CAS-Operation
        let value = self.node_id.as_bytes().to_vec();
        match self
            .client
            .try_acquire_with_lease(&self.election_key, value, lease_id)
            .await?
        {
            true => {
                // Erfolgreich! Wir sind jetzt Leader
                self.is_leader.store(true, Ordering::SeqCst);
                log_info!(
                    "etcd::ha::leader_election",
                    &format!("Successfully elected as leader! (Lease: {})", lease_id)
                );

                // Starte Lease Renewal
                self.start_lease_renewal().await;
            }
            false => {
                // Ein anderer Node war schneller
                log_warn!(
                    "etcd::ha::leader_election",
                    "Another node became leader (race condition)"
                );

                // Revoke unseren Lease da wir ihn nicht brauchen
                if let Err(e) = self.client.revoke_lease(lease_id).await {
                    log_warn!(
                        "etcd::ha::leader_election",
                        &format!("Failed to revoke unused lease: {}", e)
                    );
                }
                *self.lease_id.write().await = None;
                self.is_leader.store(false, Ordering::SeqCst);
            }
        }

        Ok(())
    }

    /// Startet Lease Renewal im Hintergrund
    async fn start_lease_renewal(&self) {
        let client = Arc::clone(&self.client);
        let lease_id = Arc::clone(&self.lease_id);
        let is_leader = Arc::clone(&self.is_leader);
        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                if let Some(lid) = *lease_id.read().await {
                    match client.keep_alive(lid).await {
                        Ok(_) => {
                            // Lease erfolgreich erneuert
                        }
                        Err(e) => {
                            log_error!(
                                "etcd::ha::leader_election",
                                &format!("Lease renewal failed: {}", e)
                            );
                            is_leader.store(false, Ordering::SeqCst);
                            log_warn!(
                                "etcd::ha::leader_election",
                                &format!("Lost leadership due to lease failure: {}", node_id)
                            );
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
        log_info!(
            "etcd::ha::leader_election",
            &format!("Watching leadership changes on: {}", key)
        );

        // Hier würde ein Watch implementiert werden
        // Für jetzt vereinfacht
        Ok(())
    }

    /// Prüft ob dieser Node Leader ist
    pub fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::SeqCst)
    }

    /// Gibt den aktuellen Leader zurück
    pub async fn get_leader(&self) -> Result<Option<String>, EtcdError> {
        match self.client.get(&self.election_key).await? {
            Some(value) => {
                let leader = String::from_utf8_lossy(&value).to_string();
                Ok(Some(leader))
            }
            None => Ok(None),
        }
    }

    /// Gibt auf Leadership auf (nur wenn wir Leader sind)
    pub async fn resign(&self) -> Result<(), EtcdError> {
        if !self.is_leader() {
            return Ok(()); // Nicht Leader, nichts zu tun
        }

        log_info!(
            "etcd::ha::leader_election",
            &format!("Resigning from leadership: {}", self.node_id)
        );

        // Revoke Lease
        if let Some(lid) = *self.lease_id.read().await {
            self.client.revoke_lease(lid).await?;
        }

        // Update State
        self.is_leader.store(false, Ordering::SeqCst);
        *self.lease_id.write().await = None;

        log_info!(
            "etcd::ha::leader_election",
            "Successfully resigned from leadership"
        );
        Ok(())
    }
}
