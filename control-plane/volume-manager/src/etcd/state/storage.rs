use super::types::*;
use crate::etcd::core::{EtcdClient, EtcdError};
use crate::log_error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

/// State Storage Layer - abstrahiert etcd operations
pub struct StateStorage {
    client: Arc<EtcdClient>,
}

impl StateStorage {
    pub fn new(client: Arc<EtcdClient>) -> Self {
        Self { client }
    }

    /// Speichert generischen State
    async fn put_state<T: Serialize>(&self, key: &str, state: &T) -> Result<(), EtcdError> {
        let data = serde_json::to_vec(state)?;
        self.client.put(key, data).await
    }

    /// Liest generischen State
    async fn get_state<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, EtcdError> {
        match self.client.get(key).await? {
            Some(data) => {
                let state = serde_json::from_slice(&data)?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Löscht State
    async fn delete_state(&self, key: &str) -> Result<(), EtcdError> {
        self.client.delete(key).await
    }

    /// Listet States mit Prefix
    async fn list_states<T: DeserializeOwned>(&self, prefix: &str) -> Result<Vec<T>, EtcdError> {
        let entries = self.client.list(prefix).await?;
        let mut states = Vec::new();

        for (_key, data) in entries {
            match serde_json::from_slice(&data) {
                Ok(state) => states.push(state),
                Err(e) => log_error!(
                    "etcd::state::storage",
                    &format!("Failed to deserialize state: {}", e)
                ),
            }
        }

        Ok(states)
    }

    // === Volume State Operations ===

    pub async fn save_volume(&self, volume: &VolumeState) -> Result<(), EtcdError> {
        let key = format!("volumes/{}", volume.id);
        self.put_state(&key, volume).await
    }

    pub async fn get_volume(&self, id: Uuid) -> Result<Option<VolumeState>, EtcdError> {
        let key = format!("volumes/{}", id);
        self.get_state(&key).await
    }

    pub async fn delete_volume(&self, id: Uuid) -> Result<(), EtcdError> {
        let key = format!("volumes/{}", id);
        self.delete_state(&key).await
    }

    pub async fn list_volumes(&self) -> Result<Vec<VolumeState>, EtcdError> {
        self.list_states("volumes/").await
    }

    // === Node State Operations ===

    pub async fn save_node(&self, node: &NodeState) -> Result<(), EtcdError> {
        let key = format!("nodes/{}", node.node_id);
        self.put_state(&key, node).await
    }

    pub async fn get_node(&self, node_id: &str) -> Result<Option<NodeState>, EtcdError> {
        let key = format!("nodes/{}", node_id);
        self.get_state(&key).await
    }

    pub async fn delete_node(&self, node_id: &str) -> Result<(), EtcdError> {
        let key = format!("nodes/{}", node_id);
        self.delete_state(&key).await
    }

    pub async fn list_nodes(&self) -> Result<Vec<NodeState>, EtcdError> {
        self.list_states("nodes/").await
    }

    // === Snapshot State Operations ===

    pub async fn save_snapshot(&self, snapshot: &SnapshotState) -> Result<(), EtcdError> {
        let key = format!("snapshots/{}", snapshot.id);
        self.put_state(&key, snapshot).await
    }

    pub async fn get_snapshot(&self, id: Uuid) -> Result<Option<SnapshotState>, EtcdError> {
        let key = format!("snapshots/{}", id);
        self.get_state(&key).await
    }

    pub async fn delete_snapshot(&self, id: Uuid) -> Result<(), EtcdError> {
        let key = format!("snapshots/{}", id);
        self.delete_state(&key).await
    }

    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotState>, EtcdError> {
        self.list_states("snapshots/").await
    }
}
