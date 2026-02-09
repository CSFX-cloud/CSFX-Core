use crate::etcd::core::EtcdClient;
use crate::etcd::state::{NodeState, NodeStatus};
use crate::{log_info, log_warn};
use std::sync::Arc;
use std::time::Duration;

/// Health Checker für Cluster Nodes
pub struct HealthChecker {
    client: Arc<EtcdClient>,
    timeout: Duration,
}

impl HealthChecker {
    pub fn new(client: Arc<EtcdClient>) -> Self {
        Self {
            client,
            timeout: Duration::from_secs(30),
        }
    }

    /// Prüft Health aller Nodes
    pub async fn check_cluster_health(&self, nodes: Vec<NodeState>) -> Vec<NodeHealthStatus> {
        let mut health_statuses = Vec::new();

        for node in nodes {
            let status = self.check_node_health(&node).await;
            health_statuses.push(status);
        }

        health_statuses
    }

    /// Prüft einzelnen Node
    async fn check_node_health(&self, node: &NodeState) -> NodeHealthStatus {
        let time_since_heartbeat = chrono::Utc::now()
            .signed_duration_since(node.last_heartbeat)
            .to_std()
            .unwrap_or(Duration::from_secs(999));

        let is_healthy = time_since_heartbeat < self.timeout;

        if !is_healthy {
            log_warn!("etcd::ha::health", &format!("Node {} is unhealthy ({}s since last heartbeat)", node.node_id, time_since_heartbeat.as_secs()));
        }

        NodeHealthStatus {
            node_id: node.node_id.clone(),
            is_healthy,
            last_heartbeat: node.last_heartbeat,
            time_since_heartbeat,
            status: node.status.clone(),
        }
    }

    /// Findet offline Nodes
    pub async fn find_offline_nodes(&self, nodes: Vec<NodeState>) -> Vec<String> {
        let health_statuses = self.check_cluster_health(nodes).await;
        health_statuses
            .into_iter()
            .filter(|s| !s.is_healthy)
            .map(|s| s.node_id)
            .collect()
    }

    /// Cluster Health Summary
    pub async fn get_cluster_summary(&self, nodes: Vec<NodeState>) -> ClusterHealthSummary {
        let health_statuses = self.check_cluster_health(nodes).await;
        let total = health_statuses.len();
        let healthy = health_statuses.iter().filter(|s| s.is_healthy).count();
        let unhealthy = total - healthy;

        log_info!("etcd::ha::health", &format!("Cluster Health: {}/{} nodes healthy", healthy, total));

        ClusterHealthSummary {
            total_nodes: total,
            healthy_nodes: healthy,
            unhealthy_nodes: unhealthy,
            nodes: health_statuses,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeHealthStatus {
    pub node_id: String,
    pub is_healthy: bool,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
    pub time_since_heartbeat: Duration,
    pub status: NodeStatus,
}

#[derive(Debug)]
pub struct ClusterHealthSummary {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub unhealthy_nodes: usize,
    pub nodes: Vec<NodeHealthStatus>,
}
