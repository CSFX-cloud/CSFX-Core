use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtcdConfig {
    /// etcd endpoints (e.g., ["http://localhost:2379"])
    pub endpoints: Vec<String>,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Request timeout
    pub request_timeout: Duration,

    /// Keepalive interval
    pub keepalive_interval: Duration,

    /// Keepalive timeout
    pub keepalive_timeout: Duration,

    /// Namespace prefix for all keys
    pub namespace: String,

    /// Username (optional)
    pub username: Option<String>,

    /// Password (optional)
    pub password: Option<String>,
}

impl Default for EtcdConfig {
    fn default() -> Self {
        Self {
            endpoints: vec!["http://localhost:2379".to_string()],
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(10),
            keepalive_interval: Duration::from_secs(30),
            keepalive_timeout: Duration::from_secs(10),
            namespace: "/csfx/volume-manager".to_string(),
            username: None,
            password: None,
        }
    }
}

impl EtcdConfig {
    /// Erstellt Config von Umgebungsvariablen
    pub fn from_env() -> Self {
        let endpoints = std::env::var("ETCD_ENDPOINTS")
            .unwrap_or_else(|_| "http://localhost:2379".to_string())
            .split(',')
            .map(|s| s.to_string())
            .collect();

        let namespace =
            std::env::var("ETCD_NAMESPACE").unwrap_or_else(|_| "/csfx/volume-manager".to_string());

        let username = std::env::var("ETCD_USERNAME").ok();
        let password = std::env::var("ETCD_PASSWORD").ok();

        Self {
            endpoints,
            namespace,
            username,
            password,
            ..Default::default()
        }
    }

    /// Erstellt vollen Key-Pfad mit Namespace
    pub fn prefixed_key(&self, key: &str) -> String {
        format!(
            "{}/{}",
            self.namespace.trim_end_matches('/'),
            key.trim_start_matches('/')
        )
    }
}
