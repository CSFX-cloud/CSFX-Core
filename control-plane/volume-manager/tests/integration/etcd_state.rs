use uuid::Uuid;
use volume_manager::etcd::core::EtcdError;
use volume_manager::etcd::state::{
    NodeRole, NodeStatus, SnapshotStatus, VolumeState, VolumeStatus,
};
use volume_manager::etcd::StateManager;

use crate::common::etcd_wrapper;

async fn manager() -> StateManager {
    StateManager::new(etcd_wrapper().await)
}

#[test]
fn volume_state_tracks_versions() {
    let mut volume = VolumeState::new("v".into(), 5, "pool".into(), true);
    assert_eq!(volume.status, VolumeStatus::Creating);
    assert_eq!(volume.version, 1);
    volume.update_status(VolumeStatus::Available);
    assert_eq!(
        (volume.status, volume.version),
        (VolumeStatus::Available, 2)
    );
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn volume_lifecycle() {
    let manager = manager().await;
    let volume = manager
        .create_volume("v".into(), 5, "pool".into(), false)
        .await
        .unwrap();
    manager
        .update_volume_status(volume.id, VolumeStatus::InUse)
        .await
        .unwrap();
    let fetched = manager.get_volume(volume.id).await.unwrap().unwrap();
    assert_eq!((fetched.status, fetched.version), (VolumeStatus::InUse, 2));
    assert_eq!(manager.list_volumes().await.unwrap().len(), 1);
    manager.delete_volume(volume.id).await.unwrap();
    assert!(manager.get_volume(volume.id).await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn updating_missing_volume_fails() {
    let error = manager()
        .await
        .update_volume_status(Uuid::new_v4(), VolumeStatus::Error)
        .await
        .unwrap_err();
    assert!(matches!(error, EtcdError::StateOperation(_)));
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn node_lifecycle() {
    let manager = manager().await;
    let node = manager
        .register_node("n1".into(), "host".into(), "10.0.0.1".into())
        .await
        .unwrap();
    assert_eq!(
        (node.status, node.role),
        (NodeStatus::Online, NodeRole::Follower)
    );
    manager.set_node_role("n1", NodeRole::Leader).await.unwrap();
    manager.mark_node_offline("n1").await.unwrap();
    assert!(manager.get_online_nodes().await.unwrap().is_empty());
    manager.update_node_heartbeat("n1").await.unwrap();
    let nodes = manager.get_online_nodes().await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].role, NodeRole::Leader);
    assert_eq!(manager.list_nodes().await.unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn missing_node_operations_fail() {
    let manager = manager().await;
    assert!(manager.update_node_heartbeat("ghost").await.is_err());
    assert!(manager.mark_node_offline("ghost").await.is_err());
    assert!(manager
        .set_node_role("ghost", NodeRole::Leader)
        .await
        .is_err());
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn snapshots_are_filtered_by_volume() {
    let manager = manager().await;
    let volume = Uuid::new_v4();
    let created = manager
        .create_snapshot(volume, "s".into(), 2)
        .await
        .unwrap();
    manager
        .create_snapshot(Uuid::new_v4(), "o".into(), 2)
        .await
        .unwrap();
    assert_eq!(created.status, SnapshotStatus::Creating);
    let listed = manager.list_volume_snapshots(volume).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn undecodable_entries_are_skipped() {
    let client = etcd_wrapper().await;
    client
        .put("volumes/broken", b"not json".to_vec())
        .await
        .unwrap();
    let manager = StateManager::new(client);
    manager
        .create_volume("ok".into(), 1, "p".into(), false)
        .await
        .unwrap();
    assert_eq!(manager.list_volumes().await.unwrap().len(), 1);
}
