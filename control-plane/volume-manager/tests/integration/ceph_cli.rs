use volume_manager::ceph::core::error::CephError;
use volume_manager::ceph::storage::types::{CephCommand, CephPool, CephVolume, HealthStatus};

use crate::common::{ceph_client, ceph_manager};

fn volume(name: &str, features: &[&str]) -> CephVolume {
    CephVolume {
        name: name.to_string(),
        pool: "csfx-volumes".to_string(),
        size_mb: 1024,
        features: features.iter().map(|f| f.to_string()).collect(),
    }
}

fn pool(name: &str) -> CephPool {
    CephPool {
        name: name.to_string(),
        pg_num: 8,
        pgp_num: 8,
        size: 3,
        min_size: 2,
    }
}

#[tokio::test]
async fn health_status_parses_cluster_summary() {
    let health = ceph_client("mon1:6789").health_status().await.unwrap();
    assert_eq!(health.status, HealthStatus::Ok);
    assert_eq!(health.mons.len(), 1);
    assert_eq!(health.mons[0].name, "a");
    assert_eq!(health.osds.len(), 2);
    assert!(health.osds[0].up && health.osds[0].in_cluster);
    assert!(!health.osds[1].up);
    assert_eq!(health.pgs.total, 10);
    assert_eq!(health.pgs.active_clean, 9);
}

#[tokio::test]
async fn health_status_maps_error_and_missing_sections() {
    let health = ceph_client("unhealthy:6789").health_status().await.unwrap();
    assert_eq!(health.status, HealthStatus::Error);
    assert!(health.mons.is_empty() && health.osds.is_empty());
    assert_eq!(health.pgs.total, 0);
}

#[tokio::test]
async fn availability_follows_command_success() {
    assert!(ceph_client("mon1:6789").is_available().await);
    assert!(!ceph_client("unreachable:6789").is_available().await);
    assert!(ceph_client("mon1:6789").wait_for_cluster(1).await.is_ok());
}

#[tokio::test]
async fn wait_for_cluster_gives_up() {
    let error = ceph_client("unreachable:6789")
        .wait_for_cluster(1)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("not available"));
}

#[tokio::test]
async fn blocklist_add_reports_failures() {
    let client = ceph_client("mon1:6789");
    client.blocklist_add("10.0.0.5:0/1").await.unwrap();
    let error = client.blocklist_add("bad:0/1").await.unwrap_err();
    assert!(format!("{error:#}").contains("Failed to blocklist"));
}

#[tokio::test]
async fn execute_surfaces_command_failures() {
    let client = ceph_client("mon1:6789");
    let error = client
        .execute(CephCommand::new("failcmd"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("ceph command failed"));
    let error = client
        .execute_rbd(CephCommand::new("failcmd"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("rbd command failed"));
}

#[tokio::test]
async fn pool_manager_lists_creates_and_deletes() {
    let manager = ceph_manager().pool_manager;
    assert_eq!(manager.list_pools().await.unwrap(), vec!["csfx-volumes"]);
    assert!(manager.pool_exists("csfx-volumes").await.unwrap());
    assert!(!manager.pool_exists("other").await.unwrap());
    manager.ensure_pool(&pool("csfx-volumes")).await.unwrap();
    manager.ensure_pool(&pool("fresh")).await.unwrap();
    manager.delete_pool("fresh").await.unwrap();
}

#[tokio::test]
async fn pool_creation_failure_is_contextualised() {
    let error = ceph_manager()
        .pool_manager
        .create_pool(&pool("failcmd"))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("Failed to create pool"));
}

#[tokio::test]
async fn rbd_manager_covers_image_and_snapshot_operations() {
    let rbd = ceph_manager().rbd_manager;
    rbd.create_image(&volume("img", &["layering"]))
        .await
        .unwrap();
    rbd.create_image(&volume("img", &[])).await.unwrap();
    rbd.resize_image("csfx-volumes", "img", 2048).await.unwrap();
    rbd.create_snapshot("csfx-volumes", "img", "s1")
        .await
        .unwrap();
    rbd.delete_snapshot("csfx-volumes", "img", "s1")
        .await
        .unwrap();
    rbd.delete_image("csfx-volumes", "img").await.unwrap();
    assert!(rbd.image_exists("csfx-volumes", "img").await.unwrap());
    assert!(!rbd.image_exists("csfx-volumes", "missing").await.unwrap());
}

#[tokio::test]
async fn rbd_manager_reports_failures() {
    let rbd = ceph_manager().rbd_manager;
    let error = rbd
        .create_image(&volume("failcreate", &[]))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("Failed to create RBD image"));
    assert!(rbd.delete_image("p", "failrm").await.is_err());
    assert!(rbd.delete_snapshot("p", "failcmd", "s").await.is_err());
    assert!(rbd.resize_image("p", "failcmd", 1).await.is_err());
}

#[tokio::test]
async fn watcher_addresses_are_listed() {
    let rbd = ceph_manager().rbd_manager;
    let watched = rbd.watcher_addresses("p", "watched").await.unwrap();
    assert_eq!(watched, vec!["10.0.0.5:0/1", "10.0.0.6:0/2"]);
    assert!(rbd.watcher_addresses("p", "idle").await.unwrap().is_empty());
    assert!(rbd.watcher_addresses("p", "nostatus").await.is_err());
    assert!(rbd.watcher_addresses("p", "badjson").await.is_err());
}

#[test]
fn ceph_command_builds_argument_vector() {
    let command = CephCommand::new("osd")
        .arg("pool")
        .args_vec(vec!["ls".to_string(), "detail".to_string()]);
    assert_eq!(command.to_vec(), vec!["osd", "pool", "ls", "detail"]);
}

#[test]
fn ceph_errors_render_messages() {
    assert_eq!(
        CephError::CommandFailed("x".into()).to_string(),
        "Command execution failed: x"
    );
    assert_eq!(
        CephError::Timeout.to_string(),
        "Timeout waiting for cluster"
    );
    let io = CephError::from(std::io::Error::other("io"));
    assert!(io.to_string().contains("IO error"));
    let json = serde_json::from_str::<u8>("x").unwrap_err();
    assert!(CephError::from(json).to_string().contains("JSON error"));
    for error in [
        CephError::ParseError("a".into()),
        CephError::UnhealthyCluster("b".into()),
        CephError::PoolError("c".into()),
        CephError::RbdError("d".into()),
        CephError::ConfigError("e".into()),
        CephError::Unknown("f".into()),
    ] {
        assert!(!error.to_string().is_empty());
    }
}
