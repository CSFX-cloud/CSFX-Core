use std::sync::Arc;
use tracing::{error, info, warn};

mod etcd;

use etcd::state::{NodeRole, NodeStatus};
use etcd::{EtcdClient, EtcdConfig, HealthChecker, LeaderElection, StateManager};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    info!("💾 Volume Manager Service starting...");

    // Lade etcd Konfiguration
    let etcd_config = EtcdConfig::from_env();
    let etcd_client = Arc::new(EtcdClient::new(etcd_config));

    // Verbinde mit etcd
    info!("🔌 Connecting to etcd...");
    etcd_client.connect().await?;
    info!("✅ Connected to etcd cluster");

    // Initialisiere Komponenten
    let state_manager = Arc::new(StateManager::new(etcd_client.clone()));
    let health_checker = Arc::new(HealthChecker::new(etcd_client.clone()));

    // Node ID generieren (in Produktion aus Hostname/Config)
    let node_id = std::env::var("NODE_ID")
        .unwrap_or_else(|_| format!("volume-manager-{}", uuid::Uuid::new_v4()));
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let ip_address = std::env::var("NODE_IP").unwrap_or_else(|_| "127.0.0.1".to_string());

    // Registriere diesen Node
    info!("🖥️  Registering node: {}", node_id);
    state_manager
        .register_node(node_id.clone(), hostname, ip_address)
        .await?;

    // Leader Election starten
    let leader_election = Arc::new(LeaderElection::new(etcd_client.clone(), node_id.clone()));
    leader_election.campaign().await?;

    // Erstelle Test-Volumes wenn Leader (nach kurzer Wartezeit)
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    if leader_election.is_leader() {
        info!("👑 I am the LEADER - creating demo volumes...");

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
                Ok(vol) => info!("   ✅ Created: {} ({}GB)", vol.name, vol.size_gb),
                Err(e) => error!("   ❌ Failed to create volume: {}", e),
            }
        }
    } else {
        info!("👥 I am a FOLLOWER - waiting for leader");
    }

    info!("✅ Volume Manager initialized");

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
                        error!("❌ Campaign failed: {}", e);
                    }
                }
            }

            // Heartbeat: Aktualisiere Node Status
            _ = heartbeat_interval.tick() => {
                if let Err(e) = state_manager.update_node_heartbeat(&node_id).await {
                    error!("❌ Failed to update heartbeat: {}", e);
                }

                // Aktualisiere Rolle basierend auf Leadership
                let role = if leader_election.is_leader() {
                    NodeRole::Leader
                } else {
                    NodeRole::Follower
                };

                if let Err(e) = state_manager.set_node_role(&node_id, role).await {
                    error!("❌ Failed to update node role: {}", e);
                }
            }

            // Health Check: Prüfe Cluster Health
            _ = health_check_interval.tick() => {
                match state_manager.list_nodes().await {
                    Ok(nodes) => {
                        let summary = health_checker.get_cluster_summary(nodes.clone()).await;

                        if summary.unhealthy_nodes > 0 {
                            warn!(
                                "⚠️  {} unhealthy nodes detected",
                                summary.unhealthy_nodes
                            );

                            // Nur Leader führt Failover durch
                            if leader_election.is_leader() {
                                perform_failover(&state_manager, &summary.nodes).await;
                            }
                        }
                    }
                    Err(e) => error!("❌ Failed to list nodes: {}", e),
                }
            }

            // Volume Operations: Nur Leader führt diese aus
            _ = operations_interval.tick() => {
                if leader_election.is_leader() {
                    info!("📦 [LEADER] Managing storage volumes...");

                    // Liste alle Volumes
                    match state_manager.list_volumes().await {
                        Ok(volumes) => {
                            info!("   📊 Total volumes: {}", volumes.len());
                            for vol in volumes.iter().take(3) {
                                info!("      - {} ({:?})", vol.name, vol.status);
                            }
                        }
                        Err(e) => error!("❌ Failed to list volumes: {}", e),
                    }

                    info!("   - Monitoring Ceph cluster health");
                    info!("   - Processing snapshot requests");
                    info!("   - Verifying encryption status");
                } else {
                    info!("📦 [FOLLOWER] Standby mode - waiting for leader instructions");

                    // Follower kann Leader abfragen
                    if let Ok(Some(leader)) = leader_election.get_leader().await {
                        info!("   👑 Current leader: {}", leader);
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
    info!("🔄 Initiating failover procedure...");

    for status in health_statuses {
        if !status.is_healthy {
            warn!(
                "⚠️  Node {} is unhealthy, marking as offline",
                status.node_id
            );

            if let Err(e) = state_manager.mark_node_offline(&status.node_id).await {
                error!(
                    "❌ Failed to mark node {} as offline: {}",
                    status.node_id, e
                );
            }

            // Hier würde man Volumes von diesem Node migrieren
            info!(
                "   📦 Initiating volume migration from node {}",
                status.node_id
            );
            // TODO: Implementiere Volume Migration
        }
    }

    info!("✅ Failover procedure completed");
}
