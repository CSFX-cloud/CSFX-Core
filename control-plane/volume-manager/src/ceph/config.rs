use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CephConfig {
    pub mon_hosts: Vec<String>,
    pub keyring_path: Option<String>,
    pub client_name: String,
    pub default_pool: String,
    pub default_pg_num: u32,
    pub default_replication: u32,
}

impl CephConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            mon_hosts: env::var("CEPH_MON_HOSTS")
                .unwrap_or_else(|_| "ceph-mon1:6789,ceph-mon2:6789,ceph-mon3:6789".to_string())
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            keyring_path: env::var("CEPH_KEYRING").ok(),
            client_name: env::var("CEPH_CLIENT_NAME").unwrap_or_else(|_| "admin".to_string()),
            default_pool: env::var("CEPH_DEFAULT_POOL")
                .unwrap_or_else(|_| "csf-volumes".to_string()),
            default_pg_num: env::var("CEPH_PG_NUM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(128),
            default_replication: env::var("CEPH_REPLICATION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
        })
    }

    pub fn mon_initial_members(&self) -> String {
        self.mon_hosts
            .iter()
            .enumerate()
            .map(|(i, _)| format!("ceph-mon{}", i + 1))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn mon_host_string(&self) -> String {
        self.mon_hosts.join(",")
    }
}

impl Default for CephConfig {
    fn default() -> Self {
        Self {
            mon_hosts: vec![
                "ceph-mon1:6789".to_string(),
                "ceph-mon2:6789".to_string(),
                "ceph-mon3:6789".to_string(),
            ],
            keyring_path: None,
            client_name: "admin".to_string(),
            default_pool: "csf-volumes".to_string(),
            default_pg_num: 128,
            default_replication: 3,
        }
    }
}
