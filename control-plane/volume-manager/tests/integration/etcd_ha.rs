use chrono::{Duration, Utc};
use volume_manager::etcd::core::EtcdError;
use volume_manager::etcd::state::{NodeRole, NodeState, NodeStatus};
use volume_manager::etcd::sync::lock::DistributedLock;
use volume_manager::etcd::sync::watcher::{StateWatcher, WatchEvent};
use volume_manager::etcd::{HealthChecker, LeaderElection};

use crate::common::etcd_wrapper;

fn node(id: &str, silent_seconds: i64) -> NodeState {
    NodeState {
        node_id: id.to_string(),
        hostname: "host".to_string(),
        ip_address: "10.0.0.1".to_string(),
        status: NodeStatus::Online,
        role: NodeRole::Follower,
        last_heartbeat: Utc::now() - Duration::seconds(silent_seconds),
        volumes: Vec::new(),
    }
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn health_checker_classifies_nodes() {
    let checker = HealthChecker::new();
    let nodes = vec![node("fresh", 1), node("stale", 120), node("future", -60)];
    let offline = checker.find_offline_nodes(nodes.clone()).await;
    assert_eq!(offline, vec!["stale".to_string(), "future".to_string()]);
    let summary = checker.get_cluster_summary(nodes).await;
    assert_eq!(summary.total_nodes, 3);
    assert_eq!(summary.healthy_nodes, 1);
    assert_eq!(summary.unhealthy_nodes, 2);
    assert_eq!(summary.nodes[0].status, NodeStatus::Online);
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn leader_election_elects_one_leader_and_resigns() {
    let client = etcd_wrapper().await;
    let first = LeaderElection::new(client.clone(), "node-a".into());
    let second = LeaderElection::new(client.clone(), "node-b".into());
    assert!(first.get_leader().await.unwrap().is_none());
    first.campaign().await.unwrap();
    first.campaign().await.unwrap();
    second.campaign().await.unwrap();
    assert!(first.is_leader());
    assert!(!second.is_leader());
    assert_eq!(second.get_leader().await.unwrap().unwrap(), "node-a");
    second.resign().await.unwrap();
    first.resign().await.unwrap();
    assert!(!first.is_leader());
    assert!(first.get_leader().await.unwrap().is_none());
    first.watch_leadership(|_| {}).await.unwrap();
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn leader_election_replaces_entry_without_lease() {
    let client = etcd_wrapper().await;
    client
        .put("election/leader", b"ghost".to_vec())
        .await
        .unwrap();
    let election = LeaderElection::new(client, "node-c".into());
    election.campaign().await.unwrap();
    assert!(election.is_leader());
    assert_eq!(election.get_leader().await.unwrap().unwrap(), "node-c");
    election.resign().await.unwrap();
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn distributed_lock_is_exclusive() {
    let client = etcd_wrapper().await;
    let mut first = DistributedLock::new(client.clone(), "job");
    let mut second = DistributedLock::new(client.clone(), "job");
    assert!(matches!(
        first.refresh().await,
        Err(EtcdError::LockFailed(_))
    ));
    first.release().await.unwrap();
    assert!(first.acquire().await.unwrap());
    first.refresh().await.unwrap();
    assert!(!second.acquire().await.unwrap());
    assert!(!second.acquire_with_retry(2, 10).await.unwrap());
    first.release().await.unwrap();
    assert!(second.acquire_with_retry(2, 10).await.unwrap());
    second.release().await.unwrap();
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn dropped_lock_does_not_panic() {
    let client = etcd_wrapper().await;
    let mut lock = DistributedLock::new(client.clone(), "dropped");
    assert!(lock.acquire().await.unwrap());
    drop(lock);
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn watcher_registration_succeeds() {
    let watcher = StateWatcher::new(etcd_wrapper().await);
    watcher.watch_prefix("volumes/", |_| {}).await.unwrap();
    watcher.watch_key("volumes/1", |_| {}).await.unwrap();
    watcher.stop_all().await.unwrap();
}

#[test]
fn watch_event_accessors() {
    let put = WatchEvent::Put {
        key: "a".into(),
        value: Vec::new(),
        version: 1,
    };
    let delete = WatchEvent::Delete { key: "b".into() };
    assert!(put.is_put() && !put.is_delete());
    assert!(delete.is_delete() && !delete.is_put());
    assert_eq!((put.key(), delete.key()), ("a", "b"));
}
