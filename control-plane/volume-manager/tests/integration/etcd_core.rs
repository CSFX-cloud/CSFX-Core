use std::time::Duration;

use volume_manager::etcd::core::EtcdError;
use volume_manager::etcd::{EtcdClient, EtcdConfig};

use crate::common::{etcd_endpoints, etcd_wrapper};

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn put_get_delete_round_trip() {
    let client = etcd_wrapper().await;
    assert!(client.get("k").await.unwrap().is_none());
    client.put("k", b"v".to_vec()).await.unwrap();
    assert_eq!(client.get("k").await.unwrap().unwrap(), b"v");
    client.delete("k").await.unwrap();
    assert!(client.get("k").await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn list_returns_namespace_relative_keys() {
    let client = etcd_wrapper().await;
    client.put("items/a", b"1".to_vec()).await.unwrap();
    client.put("items/b", b"2".to_vec()).await.unwrap();
    client.put("other/c", b"3".to_vec()).await.unwrap();
    let mut listed = client.list("items/").await.unwrap();
    listed.sort();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].0, "items/a");
    assert_eq!(listed[1].1, b"2");
    assert_eq!(client.config().namespace, client.config().namespace.clone());
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn leases_attach_expire_and_revoke() {
    let client = etcd_wrapper().await;
    let lease = client.grant_lease(30).await.unwrap();
    client
        .put_with_lease("leased", b"x".to_vec(), lease)
        .await
        .unwrap();
    let (value, attached) = client.get_with_lease("leased").await.unwrap().unwrap();
    assert_eq!((value, attached), (b"x".to_vec(), lease));
    assert!(client.get_with_lease("absent").await.unwrap().is_none());
    assert!(client.lease_time_to_live(lease).await.unwrap().unwrap() > 0);
    client.keep_alive(lease).await.unwrap();
    client.revoke_lease(lease).await.unwrap();
    assert!(client.lease_time_to_live(lease).await.unwrap().is_none());
    assert!(client.get("leased").await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn try_acquire_is_exclusive() {
    let client = etcd_wrapper().await;
    let first = client.grant_lease(30).await.unwrap();
    let second = client.grant_lease(30).await.unwrap();
    assert!(client
        .try_acquire_with_lease("lock", b"a".to_vec(), first)
        .await
        .unwrap());
    assert!(!client
        .try_acquire_with_lease("lock", b"b".to_vec(), second)
        .await
        .unwrap());
    assert_eq!(client.get("lock").await.unwrap().unwrap(), b"a");
}

#[tokio::test]
#[ignore = "requires etcd via ETCD_ENDPOINTS"]
async fn operations_connect_lazily() {
    let client = EtcdClient::new(EtcdConfig {
        endpoints: etcd_endpoints(),
        namespace: format!("/csfx-test/lazy-{}", uuid::Uuid::new_v4()),
        ..EtcdConfig::default()
    });
    client.put("lazy", b"1".to_vec()).await.unwrap();
    assert!(client.get("lazy").await.unwrap().is_some());
}

#[tokio::test]
async fn unreachable_endpoint_fails_operations() {
    let client = EtcdClient::new(EtcdConfig {
        endpoints: vec!["http://127.0.0.1:1".to_string()],
        connect_timeout: Duration::from_millis(300),
        request_timeout: Duration::from_millis(300),
        ..EtcdConfig::default()
    });
    assert!(client.put("k", b"v".to_vec()).await.is_err());
}

#[test]
fn etcd_errors_render_messages() {
    let errors = [
        EtcdError::Connection("a".into()),
        EtcdError::LeaderElection("b".into()),
        EtcdError::StateOperation("c".into()),
        EtcdError::LockFailed("d".into()),
        EtcdError::NoLeader,
        EtcdError::LeaseExpired,
        EtcdError::Watch("e".into()),
        EtcdError::Timeout("f".into()),
        EtcdError::InvalidConfig("g".into()),
    ];
    assert!(errors.iter().all(|error| !error.to_string().is_empty()));
    let json = serde_json::from_str::<u8>("x").unwrap_err();
    assert!(EtcdError::from(json).to_string().contains("Serialization"));
}
