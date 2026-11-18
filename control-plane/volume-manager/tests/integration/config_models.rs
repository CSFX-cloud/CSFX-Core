use volume_manager::ceph::core::CephConfig;
use volume_manager::ceph::ops::init_ceph;
use volume_manager::models::volume::{SnapshotStatus, VolumeStatus};

use crate::common::install_fake_ceph;

#[test]
fn volume_status_round_trips_through_strings() {
    for status in [
        VolumeStatus::Creating,
        VolumeStatus::Available,
        VolumeStatus::Attaching,
        VolumeStatus::InUse,
        VolumeStatus::Detaching,
        VolumeStatus::Deleting,
        VolumeStatus::Error,
    ] {
        assert_eq!(VolumeStatus::from_str(status.as_str()), status);
    }
    assert_eq!(VolumeStatus::from_str("unknown"), VolumeStatus::Available);
}

#[test]
fn snapshot_status_strings() {
    assert_eq!(SnapshotStatus::Available.as_str(), "available");
    assert_eq!(SnapshotStatus::Creating.as_str(), "creating");
    assert_eq!(SnapshotStatus::Deleting.as_str(), "deleting");
    assert_eq!(SnapshotStatus::Error.as_str(), "error");
}

#[test]
fn ceph_config_defaults_and_helpers() {
    let config = CephConfig::default();
    assert_eq!(
        config.mon_initial_members(),
        "ceph-mon1,ceph-mon2,ceph-mon3"
    );
    assert_eq!(
        config.mon_host_string(),
        "ceph-mon1:6789,ceph-mon2:6789,ceph-mon3:6789"
    );
}

#[tokio::test]
async fn environment_driven_initialisation() {
    install_fake_ceph();
    std::env::set_var("CEPH_MON_HOSTS", "mon1:6789,mon2:6789");
    std::env::set_var("CEPH_KEYRING", "/etc/ceph/keyring");
    std::env::set_var("CEPH_CLIENT_NAME", "csfx");
    std::env::set_var("CEPH_PG_NUM", "64");
    std::env::set_var("CEPH_REPLICATION", "2");
    std::env::set_var("CEPH_DEFAULT_POOL", "csfx-env-pool");
    let config = CephConfig::from_env().unwrap();
    assert_eq!(config.mon_hosts.len(), 2);
    assert_eq!(config.client_name, "csfx");
    assert_eq!(config.default_pg_num, 64);
    assert_eq!(config.default_replication, 2);
    assert_eq!(config.default_pool, "csfx-env-pool");

    let manager = init_ceph().await.unwrap();
    assert!(manager.client.is_available().await);
}
