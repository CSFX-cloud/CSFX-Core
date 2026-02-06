use super::{EtcdConfig, EtcdError};
use crate::{log_info, log_warn};
use etcd_client::{Client, ConnectOptions, GetOptions, PutOptions};
use std::sync::Arc;
use tokio::sync::RwLock;

/// etcd Client mit Connection Management
#[derive(Clone)]
pub struct EtcdClient {
    config: Arc<EtcdConfig>,
    client: Arc<RwLock<Option<Client>>>,
}

impl EtcdClient {
    /// Erstellt neuen etcd Client
    pub fn new(config: EtcdConfig) -> Self {
        Self {
            config: Arc::new(config),
            client: Arc::new(RwLock::new(None)),
        }
    }

    /// Verbindet mit etcd
    pub async fn connect(&self) -> Result<(), EtcdError> {
        log_info!("etcd::client", "Connecting to etcd cluster...");

        let mut options = ConnectOptions::new()
            .with_connect_timeout(self.config.connect_timeout)
            .with_timeout(self.config.request_timeout)
            .with_keep_alive(
                self.config.keepalive_interval,
                self.config.keepalive_timeout,
            );

        // Auth falls vorhanden
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            options = options.with_user(username, password);
        }

        let client = Client::connect(&self.config.endpoints, Some(options))
            .await
            .map_err(|e| EtcdError::Connection(e.to_string()))?;

        *self.client.write().await = Some(client);
        log_info!("etcd::client", "Successfully connected to etcd cluster");
        Ok(())
    }

    /// Holt etcd client oder reconnect
    async fn get_client(&self) -> Result<Client, EtcdError> {
        let read_guard = self.client.read().await;
        if let Some(client) = read_guard.as_ref() {
            return Ok(client.clone());
        }
        drop(read_guard);

        // Reconnect
        log_warn!(
            "etcd::client",
            "No active etcd connection, attempting to reconnect..."
        );
        self.connect().await?;

        self.client
            .read()
            .await
            .as_ref()
            .ok_or_else(|| EtcdError::Connection("Failed to establish connection".to_string()))
            .cloned()
    }

    /// Setzt einen Key-Value
    pub async fn put(&self, key: &str, value: Vec<u8>) -> Result<(), EtcdError> {
        let full_key = self.config.prefixed_key(key);

        let mut client = self.get_client().await?;
        client
            .put(full_key, value, None)
            .await
            .map_err(|e| EtcdError::StateOperation(e.to_string()))?;

        Ok(())
    }

    /// Setzt einen Key-Value mit Lease
    pub async fn put_with_lease(
        &self,
        key: &str,
        value: Vec<u8>,
        lease_id: i64,
    ) -> Result<(), EtcdError> {
        let full_key = self.config.prefixed_key(key);

        let mut client = self.get_client().await?;
        let options = etcd_client::PutOptions::new().with_lease(lease_id);
        client
            .put(full_key, value, Some(options))
            .await
            .map_err(|e| EtcdError::StateOperation(e.to_string()))?;

        Ok(())
    }

    /// Holt einen Wert
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, EtcdError> {
        let full_key = self.config.prefixed_key(key);

        let mut client = self.get_client().await?;
        let resp = client
            .get(full_key, None)
            .await
            .map_err(|e| EtcdError::StateOperation(e.to_string()))?;

        Ok(resp.kvs().first().map(|kv| kv.value().to_vec()))
    }

    /// Holt einen Wert mit Lease-Info
    pub async fn get_with_lease(&self, key: &str) -> Result<Option<(Vec<u8>, i64)>, EtcdError> {
        let full_key = self.config.prefixed_key(key);

        let mut client = self.get_client().await?;
        let resp = client
            .get(full_key, None)
            .await
            .map_err(|e| EtcdError::StateOperation(e.to_string()))?;

        Ok(resp
            .kvs()
            .first()
            .map(|kv| (kv.value().to_vec(), kv.lease())))
    }

    /// Prüft ob ein Lease noch gültig ist
    pub async fn lease_time_to_live(&self, lease_id: i64) -> Result<Option<i64>, EtcdError> {
        let mut client = self.get_client().await?;
        match client.lease_time_to_live(lease_id, None).await {
            Ok(resp) => {
                if resp.ttl() > 0 {
                    Ok(Some(resp.ttl()))
                } else {
                    Ok(None) // Lease expired
                }
            }
            Err(_) => Ok(None), // Lease doesn't exist anymore
        }
    }

    /// Löscht einen Key
    pub async fn delete(&self, key: &str) -> Result<(), EtcdError> {
        let full_key = self.config.prefixed_key(key);

        let mut client = self.get_client().await?;
        client
            .delete(full_key, None)
            .await
            .map_err(|e| EtcdError::StateOperation(e.to_string()))?;

        Ok(())
    }

    /// Versucht einen Key mit Lease zu setzen, nur wenn er nicht existiert (atomare CAS-Operation)
    /// Gibt true zurück wenn erfolgreich, false wenn Key bereits existiert
    pub async fn try_acquire_with_lease(
        &self,
        key: &str,
        value: Vec<u8>,
        lease_id: i64,
    ) -> Result<bool, EtcdError> {
        let full_key = self.config.prefixed_key(key);

        let mut client = self.get_client().await?;

        // Transaction: Setze Key NUR wenn er nicht existiert (Version = 0)
        use etcd_client::{Compare, CompareOp, Txn, TxnOp};

        let put_options = PutOptions::new().with_lease(lease_id);
        let compare = Compare::version(full_key.clone(), CompareOp::Equal, 0);
        let put = TxnOp::put(full_key.clone(), value, Some(put_options));
        let get = TxnOp::get(full_key.clone(), None);

        let txn = Txn::new().when([compare]).and_then([put]).or_else([get]);

        let txn_resp = client.txn(txn).await.map_err(|e| EtcdError::Client(e))?;

        // Wenn succeeded = true, war die Transaction erfolgreich (Key nicht vorhanden)
        Ok(txn_resp.succeeded())
    }

    /// Listet alle Keys mit Prefix
    pub async fn list(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, EtcdError> {
        let full_prefix = self.config.prefixed_key(prefix);

        let mut client = self.get_client().await?;
        let options = GetOptions::new().with_prefix();
        let resp = client
            .get(full_prefix.clone(), Some(options))
            .await
            .map_err(|e| EtcdError::StateOperation(e.to_string()))?;

        let results = resp
            .kvs()
            .iter()
            .map(|kv| {
                let key = String::from_utf8_lossy(kv.key()).to_string();
                let key = key
                    .trim_start_matches(&self.config.namespace)
                    .trim_start_matches('/')
                    .to_string();
                (key, kv.value().to_vec())
            })
            .collect();

        Ok(results)
    }

    /// Erstellt einen Lease (für TTL)
    pub async fn grant_lease(&self, ttl: i64) -> Result<i64, EtcdError> {
        let mut client = self.get_client().await?;
        let resp = client
            .lease_grant(ttl, None)
            .await
            .map_err(|e| EtcdError::Client(e))?;

        Ok(resp.id())
    }

    /// Erneuert einen Lease
    pub async fn keep_alive(&self, lease_id: i64) -> Result<(), EtcdError> {
        let mut client = self.get_client().await?;
        let (mut keeper, _stream) = client
            .lease_keep_alive(lease_id)
            .await
            .map_err(|e| EtcdError::Client(e))?;

        keeper
            .keep_alive()
            .await
            .map_err(|e| EtcdError::Client(e))?;
        Ok(())
    }

    /// Widerruft einen Lease
    pub async fn revoke_lease(&self, lease_id: i64) -> Result<(), EtcdError> {
        let mut client = self.get_client().await?;
        client
            .lease_revoke(lease_id)
            .await
            .map_err(|e| EtcdError::Client(e))?;

        Ok(())
    }

    /// Holt Config
    pub fn config(&self) -> &EtcdConfig {
        &self.config
    }
}
