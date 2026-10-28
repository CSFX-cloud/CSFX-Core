use anyhow::Result;
use entity::entities::{agents, garage_nodes};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use shared::leader::LeaderLease;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::{
    db::garage_nodes as garage_nodes_db,
    garage::{client::LayoutRole, GarageClient},
    log_error, log_info, log_warn,
};

const RECONCILE_INTERVAL_SECONDS: u64 = 30;
const MIN_STORAGE_NODES_FOR_FULL_REPLICATION: usize = 3;

pub fn replication_factor_for(storage_node_count: usize) -> u32 {
    if storage_node_count >= MIN_STORAGE_NODES_FOR_FULL_REPLICATION {
        3
    } else {
        1
    }
}

async fn peer_addrs(
    db: &DatabaseConnection,
    known_nodes: &[garage_nodes::Model],
) -> Result<Vec<String>> {
    let mut addrs = Vec::new();

    for node in known_nodes {
        let Some(garage_node_id) = &node.garage_node_id else {
            continue;
        };
        let Some(agent_id) = node.agent_id else {
            continue;
        };
        let Some(agent) = agents::Entity::find_by_id(agent_id).one(db).await? else {
            continue;
        };
        let Some(wg_ip) = agent.wg_tunnel_ip else {
            continue;
        };
        addrs.push(format!("{}@{}:3901", garage_node_id, wg_ip));
    }

    Ok(addrs)
}

async fn reconcile_once(db: &DatabaseConnection, garage: &GarageClient) -> Result<()> {
    let known_nodes = garage_nodes_db::list(db).await?;

    let addrs = peer_addrs(db, &known_nodes).await?;
    if let Err(e) = garage.connect_cluster_nodes(&addrs).await {
        log_warn!(
            "garage::layout",
            &format!("failed to connect garage cluster nodes err={}", e)
        );
    }

    let status = match garage.get_cluster_status().await {
        Ok(status) => status,
        Err(e) => {
            log_warn!(
                "garage::layout",
                &format!("failed to read garage cluster status err={}", e)
            );
            return Ok(());
        }
    };

    for node in &known_nodes {
        let is_up = status
            .nodes
            .iter()
            .any(|n| Some(n.id.clone()) == node.garage_node_id && n.is_up);

        if !is_up && node.status == "up" {
            log_warn!(
                "garage::layout",
                &format!("garage node reported down id={}", node.id)
            );
            garage_nodes_db::mark_down(db, node.id).await?;
        }
    }

    let storage_nodes: Vec<&garage_nodes::Model> = known_nodes
        .iter()
        .filter(|n| n.role == "storage" && n.capacity_bytes.is_some())
        .collect();

    if storage_nodes.is_empty() {
        return Ok(());
    }

    let roles: Vec<LayoutRole> = known_nodes
        .iter()
        .filter_map(|n| {
            n.garage_node_id.as_ref().map(|garage_id| LayoutRole {
                id: garage_id.clone(),
                zone: n.zone.clone(),
                capacity: n.capacity_bytes,
                tags: vec![],
            })
        })
        .collect();

    if roles.is_empty() {
        return Ok(());
    }

    if let Err(e) = garage.update_cluster_layout(roles).await {
        log_error!(
            "garage::layout",
            &format!("failed to stage cluster layout err={}", e)
        );
        return Ok(());
    }

    if let Err(e) = garage.apply_cluster_layout(status.layout_version + 1).await {
        log_error!(
            "garage::layout",
            &format!("failed to apply cluster layout err={}", e)
        );
        return Ok(());
    }

    log_info!(
        "garage::layout",
        &format!(
            "applied cluster layout storage_nodes={} replication_factor={}",
            storage_nodes.len(),
            replication_factor_for(storage_nodes.len())
        )
    );
    Ok(())
}

pub async fn run_reconcile_loop(db: DatabaseConnection, garage: GarageClient, leader: LeaderLease) {
    loop {
        sleep(Duration::from_secs(RECONCILE_INTERVAL_SECONDS)).await;

        if !leader.is_leader() {
            continue;
        }

        if let Err(e) = reconcile_once(&db, &garage).await {
            log_error!(
                "garage::layout",
                &format!("reconcile loop iteration failed err={}", e)
            );
        }
    }
}

const SELF_REGISTER_RETRY_SECONDS: u64 = 5;
const SELF_REGISTER_MAX_ATTEMPTS: u32 = 60;

fn available_capacity_bytes(data_dir: &str) -> Option<i64> {
    match nix::sys::statvfs::statvfs(data_dir) {
        Ok(stats) => {
            let bytes = stats.blocks_available() * stats.fragment_size();
            Some(bytes as i64)
        }
        Err(e) => {
            log_warn!(
                "garage::layout",
                &format!(
                    "failed to read available capacity path={} err={}",
                    data_dir, e
                )
            );
            None
        }
    }
}

async fn agent_id_for_addr(db: &DatabaseConnection, addr: Option<&str>) -> Result<Option<Uuid>> {
    let Some((ip, _port)) = addr.and_then(|addr| addr.rsplit_once(':')) else {
        return Ok(None);
    };
    let agent = agents::Entity::find()
        .filter(agents::Column::WgTunnelIp.eq(ip))
        .one(db)
        .await?;
    Ok(agent.map(|agent| agent.id))
}

pub async fn register_self_as_node(
    db: &DatabaseConnection,
    garage: &GarageClient,
    zone: &str,
    data_dir: &str,
) {
    for attempt in 1..=SELF_REGISTER_MAX_ATTEMPTS {
        let status = match garage.get_cluster_status().await {
            Ok(status) => garage.get_self_node_id().await.map(|id| (status, id)),
            Err(e) => Err(e),
        };
        match status {
            Ok((status, self_id)) => {
                let Some(self_node) = status.nodes.iter().find(|node| node.id == self_id) else {
                    log_warn!(
                        "garage::layout",
                        "garage cluster status does not list the local node yet"
                    );
                    sleep(Duration::from_secs(SELF_REGISTER_RETRY_SECONDS)).await;
                    continue;
                };
                let agent_id = agent_id_for_addr(db, self_node.addr.as_deref())
                    .await
                    .unwrap_or(None);

                let capacity_bytes = self_node
                    .role
                    .as_ref()
                    .and_then(|r| r.capacity)
                    .or_else(|| available_capacity_bytes(data_dir));
                let node_zone = self_node
                    .role
                    .as_ref()
                    .map(|r| r.zone.as_str())
                    .filter(|z| !z.is_empty())
                    .unwrap_or(zone);

                match crate::db::garage_nodes::upsert_self(
                    db,
                    &self_node.id,
                    agent_id,
                    node_zone,
                    capacity_bytes,
                )
                .await
                {
                    Ok(node) => {
                        log_info!(
                            "garage::layout",
                            &format!("registered self as garage node id={}", node.id)
                        );
                        return;
                    }
                    Err(e) => {
                        log_error!(
                            "garage::layout",
                            &format!("failed to persist self garage node registration err={}", e)
                        );
                        return;
                    }
                }
            }
            Err(e) => {
                log_warn!(
                    "garage::layout",
                    &format!(
                        "self-registration attempt {}/{} failed err={}",
                        attempt, SELF_REGISTER_MAX_ATTEMPTS, e
                    )
                );
                sleep(Duration::from_secs(SELF_REGISTER_RETRY_SECONDS)).await;
            }
        }
    }

    log_error!(
        "garage::layout",
        "giving up on self garage node registration after max attempts"
    );
}
