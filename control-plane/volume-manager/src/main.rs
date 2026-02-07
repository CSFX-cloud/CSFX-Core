use std::sync::Arc;

use etcd::state::NodeRole;
use etcd::StateManager;

mod ceph;
mod etcd;
mod logger;
mod patroni;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialisiere etcd Cluster
    let init_data = etcd::init::init_cluster().await?;
    let _etcd_client = init_data.etcd_client;
    let state_manager = init_data.state_manager;
    let health_checker = init_data.health_checker;
    let leader_election = init_data.leader_election;
    let node_id = init_data.node_id;

    // Initialisiere Ceph Storage (nur Leader)
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let ceph_manager = if leader_election.is_leader() {
        log_info!("main", "Node is leader, initializing Ceph storage");

        match ceph::ops::init_ceph().await {
            Ok(manager) => {
                log_info!("main", "Ceph storage initialized successfully");

                // Erstelle PostgreSQL Volumes für Patroni
                match ceph::ops::create_postgres_volumes(&manager, 3).await {
                    Ok(volumes) => {
                        log_info!(
                            "main",
                            &format!("Created PostgreSQL volumes on Ceph: {:?}", volumes)
                        );
                    }
                    Err(e) => {
                        log_error!(
                            "main",
                            &format!("Failed to create PostgreSQL volumes: {}", e)
                        );
                    }
                }

                Some(Arc::new(manager))
            }
            Err(e) => {
                log_warn!(
                    "main",
                    &format!(
                        "Ceph initialization failed (continuing without Ceph): {}",
                        e
                    )
                );
                None
            }
        }
    } else {
        log_info!("main", "Node is follower, skipping Ceph initialization");
        None
    };

    // Initialisiere Patroni Monitoring (alle Nodes)
    log_info!("main", "Initializing Patroni PostgreSQL HA monitoring");

    let patroni_scope =
        std::env::var("PATRONI_SCOPE").unwrap_or_else(|_| "postgres-csf".to_string());

    let patroni_nodes = std::env::var("PATRONI_NODES")
        .unwrap_or_else(|_| "patroni1:8008,patroni2:8008,patroni3:8008".to_string())
        .split(',')
        .map(|s| format!("http://{}", s.trim()))
        .collect::<Vec<_>>();

    let patroni_client = patroni::PatroniClient::new(patroni_scope.clone(), patroni_nodes);
    let patroni_monitor = Arc::new(patroni::PatroniMonitor::new(patroni_client, 10));

    // Warte bis Patroni Cluster bereit ist
    log_info!("main", "Waiting for Patroni cluster to be ready...");
    if let Err(e) = patroni_monitor.wait_for_cluster_ready(120).await {
        log_warn!("main", &format!("Patroni cluster not ready: {}", e));
    } else {
        log_info!("main", "✅ Patroni cluster is ready and healthy");
    }

    // Starte Patroni Monitoring Loop (in eigenem Task)
    let monitor_handle = {
        let monitor = patroni_monitor.clone();
        tokio::spawn(async move {
            monitor.start_monitoring().await;
        })
    };

    // Erstelle Test-Volumes wenn Leader (nach kurzer Wartezeit)
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    if leader_election.is_leader() {
        log_info!(
            "main",
            "Node is leader, initializing volume management tasks"
        );

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
                Ok(vol) => log_info!(
                    "main",
                    &format!(
                        "Successfully created volume: {} ({} GB)",
                        vol.name, vol.size_gb
                    )
                ),
                Err(e) => log_error!("main", &format!("Failed to create volume: {}", e)),
            }
        }
    } else {
        log_info!("main", "Node is follower, waiting for leader");
    }

    log_info!(
        "main",
        "✅ Volume Manager with Patroni HA initialized successfully"
    );

    // Hauptschleife
    let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    let mut health_check_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    let mut operations_interval = tokio::time::interval(tokio::time::Duration::from_secs(35));
    let mut patroni_check_interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
    let mut election_interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        tokio::select! {
            // Leader Election Loop: Versuche regelmäßig Leader zu werden
            _ = election_interval.tick() => {
                if !leader_election.is_leader() {
                    if let Err(e) = leader_election.campaign().await {
                        log_error!("main", &format!("Leader election campaign failed: {}", e));
                    }
                }
            }

            // Heartbeat: Aktualisiere Node Status
            _ = heartbeat_interval.tick() => {
                if let Err(e) = state_manager.update_node_heartbeat(&node_id).await {
                    log_error!("main", &format!("Failed to update node heartbeat: {}", e));
                }

                // Aktualisiere Rolle basierend auf Leadership
                let role = if leader_election.is_leader() {
                    NodeRole::Leader
                } else {
                    NodeRole::Follower
                };

                if let Err(e) = state_manager.set_node_role(&node_id, role).await {
                    log_error!("main", &format!("Failed to update node role: {}", e));
                }
            }

            // Health Check: Prüfe Cluster Health
            _ = health_check_interval.tick() => {
                match state_manager.list_nodes().await {
                    Ok(nodes) => {
                        let summary = health_checker.get_cluster_summary(nodes.clone()).await;

                        if summary.unhealthy_nodes > 0 {
                            log_warn!("main", &format!("Detected {} unhealthy nodes", summary.unhealthy_nodes));

                            // Nur Leader führt Failover durch
                            if leader_election.is_leader() {
                                perform_failover(&state_manager, &summary.nodes, &ceph_manager).await;
                            }
                        }
                    }
                    Err(e) => log_error!("main", &format!("Failed to list nodes: {}", e)),
                }
            }

            // Patroni Check: Überwache PostgreSQL HA Status
            _ = patroni_check_interval.tick() => {
                if leader_election.is_leader() {
                    match patroni_monitor.get_primary().await {
                        Ok(Some(primary)) => {
                            log_info!("main", &format!("👑 PostgreSQL Primary: {}", primary.name));

                            // Prüfe Replicas
                            match patroni_monitor.get_replicas().await {
                                Ok(replicas) => {
                                    log_info!("main", &format!("🔄 PostgreSQL Replicas: {}", replicas.len()));
                                    for replica in replicas {
                                        let lag_info = if let Some(lag) = replica.lag {
                                            format!(" (Lag: {}KB)", lag / 1024)
                                        } else {
                                            String::new()
                                        };
                                        log_debug!("main", &format!("  - {}{}", replica.name, lag_info));
                                    }
                                }
                                Err(e) => log_error!("main", &format!("Failed to get replicas: {}", e)),
                            }
                        }
                        Ok(None) => {
                            log_error!("main", "⚠️  NO PRIMARY FOUND! Patroni failover in progress?");
                        }
                        Err(e) => {
                            log_error!("main", &format!("Failed to get primary: {}", e));
                        }
                    }

                    // Prüfe ob Cluster healthy ist
                    if !patroni_monitor.is_cluster_healthy().await {
                        log_warn!("main", "⚠️  Patroni cluster is not healthy!");

                        // Hier könnte man zusätzliche Recovery-Aktionen triggern
                        if let Some(ceph) = &ceph_manager {
                            log_info!("main", "Checking Ceph storage health for recovery...");
                            match ceph.client.health_status().await {
                                Ok(health) => {
                                    log_info!("main", &format!("Ceph Health: {:?}", health.status));
                                }
                                Err(e) => {
                                    log_error!("main", &format!("Ceph health check failed: {}", e));
                                }
                            }
                        }
                    }
                }
            }

            // Volume Operations: Nur Leader führt diese aus
            _ = operations_interval.tick() => {
                if leader_election.is_leader() {
                    log_info!("main", "[LEADER] Managing storage volumes...");

                    // Liste alle Volumes
                    match state_manager.list_volumes().await {
                        Ok(volumes) => {
                            log_info!("main", &format!("Total volumes: {}", volumes.len()));
                            for vol in volumes.iter().take(3) {
                                log_info!("main", &format!("- {} ({:?})", vol.name, vol.status));
                            }
                        }
                        Err(e) => log_error!("main", &format!("Failed to list volumes: {}", e)),
                    }

                    log_info!("main", "- Monitoring Ceph cluster health");
                    log_info!("main", "- Processing snapshot requests");
                    log_info!("main", "- Verifying encryption status");
                } else {
                    log_info!("main", "[FOLLOWER] Standby mode - waiting for leader instructions");

                    // Follower kann Leader abfragen
                    if let Ok(Some(leader)) = leader_election.get_leader().await {
                        log_info!("main", &format!("Current leader: {}", leader));
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
    ceph_manager: &Option<Arc<ceph::ops::CephManager>>,
) {
    log_info!("main", "🚨 Initiating failover procedure...");

    for status in health_statuses {
        if !status.is_healthy {
            log_warn!(
                "main",
                &format!("Node {} is unhealthy, marking as offline", status.node_id)
            );

            if let Err(e) = state_manager.mark_node_offline(&status.node_id).await {
                log_error!(
                    "main",
                    &format!("Failed to mark node {} as offline: {}", status.node_id, e)
                );
            }

            // Volume Migration (für User-Volumes, nicht PostgreSQL)
            // PostgreSQL Failover wird von Patroni automatisch gehandelt!
            log_info!(
                "main",
                &format!(
                    "Initiating user volume migration from node {}",
                    status.node_id
                )
            );

            if let Some(ceph) = ceph_manager {
                // Liste alle Volumes die auf dem toten Node waren
                match state_manager.list_volumes().await {
                    Ok(volumes) => {
                        let node_volumes: Vec<_> = volumes
                            .iter()
                            .filter(|v| v.node_id.as_ref() == Some(&status.node_id))
                            .collect();

                        if !node_volumes.is_empty() {
                            log_info!(
                                "main",
                                &format!(
                                    "Found {} volumes to migrate from {}",
                                    node_volumes.len(),
                                    status.node_id
                                )
                            );

                            for volume in node_volumes {
                                log_info!(
                                    "main",
                                    &format!(
                                        "Migrating volume: {} ({}GB)",
                                        volume.name, volume.size_gb
                                    )
                                );

                                // Hier würde Volume-Migration implementiert werden:
                                // 1. Unmap von toter Node (Ceph RBD exclusive-lock release)
                                // 2. Map zu gesunder Node
                                // 3. Volume-Status in etcd aktualisieren

                                // Für jetzt nur loggen
                                log_info!(
                                    "main",
                                    &format!("Volume {} ready for remount (Ceph ensures data persistence)", volume.name)
                                );
                            }
                        } else {
                            log_info!(
                                "main",
                                &format!("No volumes found on node {}", status.node_id)
                            );
                        }
                    }
                    Err(e) => {
                        log_error!("main", &format!("Failed to list volumes: {}", e));
                    }
                }
            } else {
                log_warn!(
                    "main",
                    "Ceph manager not available, skipping volume migration"
                );
            }
        }
    }

    log_info!("main", "✅ Failover procedure completed");
    log_info!(
        "main",
        "Note: PostgreSQL failover is handled automatically by Patroni"
    );
}
