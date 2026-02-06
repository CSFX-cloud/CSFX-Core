use std::sync::Arc;
use tracing::{error, info, warn};

use etcd::state::NodeRole;
use etcd::StateManager;

mod etcd;
mod logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let init_data = etcd::init::init_cluster().await?;
    let etcd_client = init_data.etcd_client;
    let state_manager = init_data.state_manager;
    let health_checker = init_data.health_checker;
    let leader_election = init_data.leader_election;
    let node_id = init_data.node_id;

    // Erstelle Test-Volumes wenn Leader (nach kurzer Wartezeit)
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    if leader_election.is_leader() {
        info!("Node is leader, creating demo volumes...");

        // Erstelle Demo-Volumes
        for i in 1..=3 {
            match state_manager
                .create_volume(
                    format!("demo-volume-{}", i),
                    100 + (i * 50),
                    "rbd".to_string(),
                    i % 2 == 0, // Jedes zweite Volume verschlüsselt
                )
                .await
            {
                Ok(vol) => info!(
                    "   Successfully created volume: {} ({} GB)",
                    vol.name, vol.size_gb
                ),
                Err(e) => error!("   Failed to create volume: {}", e),
            }
        }
    } else {
        info!("Node is follower, waiting for leader");
    }

    info!("Volume Manager initialized successfully");

    // Hauptschleife
    let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    let mut health_check_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    let mut operations_interval = tokio::time::interval(tokio::time::Duration::from_secs(35));
    let mut election_interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        tokio::select! {
            // Leader Election Loop: Versuche regelmäßig Leader zu werden
            _ = election_interval.tick() => {
                if !leader_election.is_leader() {
                    if let Err(e) = leader_election.campaign().await {
                        error!("Leader election campaign failed: {}", e);
                    }
                }
            }

            // Heartbeat: Aktualisiere Node Status
            _ = heartbeat_interval.tick() => {
                if let Err(e) = state_manager.update_node_heartbeat(&node_id).await {
                    error!("Failed to update node heartbeat: {}", e);
                }

                // Aktualisiere Rolle basierend auf Leadership
                let role = if leader_election.is_leader() {
                    NodeRole::Leader
                } else {
                    NodeRole::Follower
                };

                if let Err(e) = state_manager.set_node_role(&node_id, role).await {
                    error!("Failed to update node role: {}", e);
                }
            }

            // Health Check: Prüfe Cluster Health
            _ = health_check_interval.tick() => {
                match state_manager.list_nodes().await {
                    Ok(nodes) => {
                        let summary = health_checker.get_cluster_summary(nodes.clone()).await;

                        if summary.unhealthy_nodes > 0 {
                            warn!(
                                "Detected {} unhealthy nodes",
                                summary.unhealthy_nodes
                            );

                            // Nur Leader führt Failover durch
                            if leader_election.is_leader() {
                                perform_failover(&state_manager, &summary.nodes).await;
                            }
                        }
                    }
                    Err(e) => error!("Failed to list nodes: {}", e),
                }
            }

            // Volume Operations: Nur Leader führt diese aus
            _ = operations_interval.tick() => {
                if leader_election.is_leader() {
                    info!("[LEADER] Managing storage volumes...");

                    // Liste alle Volumes
                    match state_manager.list_volumes().await {
                        Ok(volumes) => {
                            info!("   Total volumes: {}", volumes.len());
                            for vol in volumes.iter().take(3) {
                                info!("      - {} ({:?})", vol.name, vol.status);
                            }
                        }
                        Err(e) => error!("Failed to list volumes: {}", e),
                    }

                    info!("   - Monitoring Ceph cluster health");
                    info!("   - Processing snapshot requests");
                    info!("   - Verifying encryption status");
                } else {
                    info!("[FOLLOWER] Standby mode - waiting for leader instructions");

                    // Follower kann Leader abfragen
                    if let Ok(Some(leader)) = leader_election.get_leader().await {
                        info!("   Current leader: {}", leader);
                    }
                }
            }
        }
    }
}

/// Führt Failover für offline Nodes durch
async fn perform_failover(
    state_manager: &Arc<StateManager>,
    health_statuses: &[etcd::ha::NodeHealthStatus],
) {
    info!("Initiating failover procedure...");

    for status in health_statuses {
        if !status.is_healthy {
            warn!("Node {} is unhealthy, marking as offline", status.node_id);

            if let Err(e) = state_manager.mark_node_offline(&status.node_id).await {
                error!("Failed to mark node {} as offline: {}", status.node_id, e);
            }

            // Hier würde man Volumes von diesem Node migrieren
            info!(
                "   Initiating volume migration from node {}",
                status.node_id
            );
            // TODO: Implementiere Volume Migration
        }
    }

    info!("Failover procedure completed successfully");
}
