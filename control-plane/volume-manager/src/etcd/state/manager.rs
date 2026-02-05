use super::{storage::StateStorage, types::*};
use crate::etcd::core::{EtcdClient, EtcdError};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// High-level State Manager
pub struct StateManager {
    storage: Arc<StateStorage>,
}

impl StateManager {
    pub fn new(client: Arc<EtcdClient>) -> Self {
        let storage = Arc::new(StateStorage::new(client));
        Self { storage }
    }

    // === Volume Management ===

    /// Erstellt neues Volume
    pub async fn create_volume(
        &self,
        name: String,
        size_gb: u64,
        pool: String,
        encrypted: bool,
    ) -> Result<VolumeState, EtcdError> {
        let volume = VolumeState::new(name, size_gb, pool, encrypted);
        info!("📦 Creating volume: {} ({})", volume.name, volume.id);
        self.storage.save_volume(&volume).await?;
        Ok(volume)
    }

    /// Aktualisiert Volume Status
    pub async fn update_volume_status(
        &self,
        id: Uuid,
        status: VolumeStatus,
    ) -> Result<(), EtcdError> {
        let mut volume = self
            .storage
            .get_volume(id)
            .await?
            .ok_or_else(|| EtcdError::StateOperation(format!("Volume {} not found", id)))?;

        volume.update_status(status);
        self.storage.save_volume(&volume).await?;
        info!("✅ Updated volume {} status to {:?}", id, volume.status);
        Ok(())
    }

    /// Holt Volume
    pub async fn get_volume(&self, id: Uuid) -> Result<Option<VolumeState>, EtcdError> {
        self.storage.get_volume(id).await
    }

    /// Listet alle Volumes
    pub async fn list_volumes(&self) -> Result<Vec<VolumeState>, EtcdError> {
        self.storage.list_volumes().await
    }

    /// Löscht Volume
    pub async fn delete_volume(&self, id: Uuid) -> Result<(), EtcdError> {
        info!("🗑️  Deleting volume: {}", id);
        self.storage.delete_volume(id).await
    }

    // === Node Management ===

    /// Registriert Node
    pub async fn register_node(
        &self,
        node_id: String,
        hostname: String,
        ip_address: String,
    ) -> Result<NodeState, EtcdError> {
        let node = NodeState {
            node_id: node_id.clone(),
            hostname,
            ip_address,
            status: NodeStatus::Online,
            role: NodeRole::Follower,
            last_heartbeat: chrono::Utc::now(),
            volumes: Vec::new(),
        };

        info!("🖥️  Registering node: {}", node_id);
        self.storage.save_node(&node).await?;
        Ok(node)
    }

    /// Aktualisiert Node Heartbeat
    pub async fn update_node_heartbeat(&self, node_id: &str) -> Result<(), EtcdError> {
        let mut node = self
            .storage
            .get_node(node_id)
            .await?
            .ok_or_else(|| EtcdError::StateOperation(format!("Node {} not found", node_id)))?;

        node.last_heartbeat = chrono::Utc::now();
        node.status = NodeStatus::Online;
        self.storage.save_node(&node).await
    }

    /// Markiert Node als Offline
    pub async fn mark_node_offline(&self, node_id: &str) -> Result<(), EtcdError> {
        let mut node = self
            .storage
            .get_node(node_id)
            .await?
            .ok_or_else(|| EtcdError::StateOperation(format!("Node {} not found", node_id)))?;

        warn!("⚠️  Marking node {} as offline", node_id);
        node.status = NodeStatus::Offline;
        self.storage.save_node(&node).await
    }

    /// Setzt Node Rolle
    pub async fn set_node_role(&self, node_id: &str, role: NodeRole) -> Result<(), EtcdError> {
        let mut node = self
            .storage
            .get_node(node_id)
            .await?
            .ok_or_else(|| EtcdError::StateOperation(format!("Node {} not found", node_id)))?;

        node.role = role;
        self.storage.save_node(&node).await?;
        info!("👑 Set node {} role to {:?}", node_id, node.role);
        Ok(())
    }

    /// Listet alle Nodes
    pub async fn list_nodes(&self) -> Result<Vec<NodeState>, EtcdError> {
        self.storage.list_nodes().await
    }

    /// Findet Online Nodes
    pub async fn get_online_nodes(&self) -> Result<Vec<NodeState>, EtcdError> {
        let nodes = self.storage.list_nodes().await?;
        Ok(nodes
            .into_iter()
            .filter(|n| n.status == NodeStatus::Online)
            .collect())
    }

    // === Snapshot Management ===

    /// Erstellt Snapshot
    pub async fn create_snapshot(
        &self,
        volume_id: Uuid,
        name: String,
        size_gb: u64,
    ) -> Result<SnapshotState, EtcdError> {
        let snapshot = SnapshotState {
            id: Uuid::new_v4(),
            volume_id,
            name: name.clone(),
            size_gb,
            status: SnapshotStatus::Creating,
            created_at: chrono::Utc::now(),
        };

        info!("📸 Creating snapshot: {} for volume {}", name, volume_id);
        self.storage.save_snapshot(&snapshot).await?;
        Ok(snapshot)
    }

    /// Listet Snapshots für Volume
    pub async fn list_volume_snapshots(
        &self,
        volume_id: Uuid,
    ) -> Result<Vec<SnapshotState>, EtcdError> {
        let snapshots = self.storage.list_snapshots().await?;
        Ok(snapshots
            .into_iter()
            .filter(|s| s.volume_id == volume_id)
            .collect())
    }
}
