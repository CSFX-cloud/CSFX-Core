use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use axum::Router;
use chrono::Utc;
use entity::entities::agents;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
use uuid::Uuid;
use volume_manager::ceph::core::{CephClient, CephConfig};
use volume_manager::ceph::ops::CephManager;
use volume_manager::ceph::storage::{PoolManager, RbdManager};
use volume_manager::server::{create_router, AppState};
use volume_manager::services::volume::VolumeService;

static FAKE_BIN: OnceLock<PathBuf> = OnceLock::new();

const FAKE_CEPH: &str = r#"#!/bin/sh
case "$*" in
*failcmd*) echo "boom" >&2; exit 1;;
*unreachable*) echo "no route" >&2; exit 1;;
*"blocklist add bad"*) echo "denied" >&2; exit 1;;
*"pool ls"*) echo '["csfx-volumes"]';;
*unhealthy*status*) echo '{"health":{"status":"HEALTH_ERR"}}';;
*status*) echo '{"health":{"status":"HEALTH_OK"},"monmap":{"mons":[{"name":"a","addr":"10.0.0.1","rank":0},{"addr":"x"}]},"osdmap":{"osds":[{"osd":0,"up":1,"in":1,"weight":1.0},{"osd":1,"up":0,"in":0,"weight":0.5}]},"pgmap":{"num_pgs":10,"pgs_by_state":[{"state_name":"active+clean","count":9}]}}';;
esac
"#;

const FAKE_RBD: &str = r#"#!/bin/sh
[ -n "$FAKE_RBD_LOG" ] && echo "$*" >> "$FAKE_RBD_LOG"
case "$*" in
*failcmd*) echo "boom" >&2; exit 1;;
*" export "*failexport*) echo "export broke" >&2; exit 1;;
*" export "*) yes csfx | head -c 2500000;;
*" import "*failimport*) cat >/dev/null; echo "import broke" >&2; exit 1;;
*" import "*) cat | wc -c > "$FAKE_RBD_IMPORT_BYTES";;
*" info "*missing*) echo "rbd: error opening image: (2) No such file or directory" >&2; exit 2;;
*" create "*failcreate*) echo "no space" >&2; exit 1;;
*" rm "*failrm*) echo "busy" >&2; exit 1;;
*" snap create "*failsnap*) echo "snap" >&2; exit 1;;
*" status "*nostatus*) echo "gone" >&2; exit 1;;
*" status "*badjson*) echo "garbage";;
*" status "*watchedbad*) echo '{"watchers":[{"address":"bad:0/1"}]}';;
*" status "*watched*) echo '{"watchers":[{"address":"10.0.0.5:0/1"},{"address":"10.0.0.6:0/2"}]}';;
*" status "*) echo '{"watchers":[]}';;
esac
"#;

fn write_script(dir: &std::path::Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("script must be written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("script must be executable");
}

pub fn install_fake_ceph() {
    FAKE_BIN.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("csfx-vm-fake-ceph-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fake bin dir must be created");
        write_script(&dir, "ceph", FAKE_CEPH);
        write_script(&dir, "rbd", FAKE_RBD);
        let path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{}", dir.display(), path));
        dir
    });
}

pub fn ceph_config(mon_host: &str) -> CephConfig {
    CephConfig {
        mon_hosts: vec![mon_host.to_string()],
        keyring_path: Some("/etc/ceph/keyring".to_string()),
        ..CephConfig::default()
    }
}

pub fn ceph_client(mon_host: &str) -> CephClient {
    install_fake_ceph();
    CephClient::new(ceph_config(mon_host))
}

pub fn ceph_manager() -> CephManager {
    let client = ceph_client("mon1:6789");
    CephManager {
        pool_manager: PoolManager::new(client.clone()),
        rbd_manager: RbdManager::new(client.clone()),
        client,
    }
}

pub fn service(db: DatabaseConnection, ceph: Option<CephManager>) -> Arc<VolumeService> {
    Arc::new(VolumeService::new(db, ceph.map(Arc::new)))
}

pub async fn router(with_ceph: bool) -> Router {
    let db = migrated_database().await;
    let ceph = with_ceph.then(ceph_manager);
    let volume_service = service(db.clone(), ceph);
    create_router(AppState { db, volume_service })
}

pub async fn insert_agent(db: &DatabaseConnection) -> Uuid {
    let id = Uuid::new_v4();
    agents::ActiveModel {
        id: Set(id),
        name: Set(format!("agent-{}", id.simple())),
        hostname: Set(format!("host-{}", id.simple())),
        ip_address: Set(None),
        agent_version: Set("0.2.2".to_string()),
        os_type: Set("linux".to_string()),
        os_version: Set("6.1".to_string()),
        architecture: Set("x86_64".to_string()),
        status: Set("online".to_string()),
        last_heartbeat: Set(None),
        registered_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
        organization_id: Set(None),
        tags: Set(None),
        capabilities: Set(None),
        public_key_pem: Set(None),
        wg_public_key: Set(None),
        wg_endpoint: Set(None),
        wg_tunnel_ip: Set(None),
        kvm_capable: Set(false),
        ssh_host_key: Set(None),
        cordoned: Set(false),
        maintenance_until: Set(None),
        role: Set("worker".to_string()),
        machine_id: Set(None),
        cpu_overcommit_ratio: Set(1.0),
        memory_overcommit_ratio: Set(1.0),
        disk_overcommit_ratio: Set(1.0),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("agent must be inserted");
    id
}

pub use test_support::{call, migrated_database};
