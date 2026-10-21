use std::collections::HashSet;

use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use serde_json::Value;
use uuid::Uuid;

use entity::entities::workloads;

use crate::models::workload::PortMapping;

const DEFAULT_NODE_PORT_RANGE_START: u16 = 30000;
const DEFAULT_NODE_PORT_RANGE_END: u16 = 32767;

fn node_port_range() -> (u16, u16) {
    let start = std::env::var("NODE_PORT_RANGE_START")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_NODE_PORT_RANGE_START);
    let end = std::env::var("NODE_PORT_RANGE_END")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_NODE_PORT_RANGE_END);
    (start, end)
}

async fn used_node_ports_on_agent(
    db: &DatabaseConnection,
    agent_id: Uuid,
    exclude_workload_id: Uuid,
) -> Result<HashSet<u16>, DbErr> {
    let rows = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.eq(agent_id))
        .filter(workloads::Column::DesiredState.ne("stopped"))
        .filter(workloads::Column::Id.ne(exclude_workload_id))
        .all(db)
        .await?;

    let mut used = HashSet::new();
    for row in rows {
        let Some(ports_json) = row.ports else {
            continue;
        };
        let Ok(mappings) = serde_json::from_value::<Vec<PortMapping>>(ports_json) else {
            continue;
        };
        for mapping in mappings {
            if let Some(node_port) = mapping.node_port {
                used.insert(node_port);
            }
        }
    }
    Ok(used)
}

fn pick_free_port(used: &HashSet<u16>, requested: &HashSet<u16>) -> Result<u16, String> {
    let (start, end) = node_port_range();
    (start..=end)
        .find(|port| !used.contains(port) && !requested.contains(port))
        .ok_or_else(|| "no free node port available in configured range".to_string())
}

fn ensure_requested_free(ports: &[PortMapping], used: &HashSet<u16>) -> Result<(), String> {
    match ports
        .iter()
        .filter_map(|mapping| mapping.node_port)
        .find(|port| used.contains(port))
    {
        Some(port) => Err(format!("node port {port} is already in use on the agent")),
        None => Ok(()),
    }
}

pub async fn allocate_node_ports(
    db: &DatabaseConnection,
    agent_id: Uuid,
    workload_id: Uuid,
    ports: &mut [PortMapping],
) -> Result<(), String> {
    if ports.is_empty() {
        return Ok(());
    }

    let mut used = used_node_ports_on_agent(db, agent_id, workload_id)
        .await
        .map_err(|e| format!("failed to query used node ports: {}", e))?;
    ensure_requested_free(ports, &used)?;
    let mut requested: HashSet<u16> = ports.iter().filter_map(|p| p.node_port).collect();

    for mapping in ports.iter_mut() {
        if mapping.node_port.is_some() {
            continue;
        }
        let port = pick_free_port(&used, &requested)?;
        mapping.node_port = Some(port);
        used.insert(port);
        requested.insert(port);
    }

    Ok(())
}

pub fn ports_to_json(ports: &[PortMapping]) -> Result<Value, String> {
    serde_json::to_value(ports).map_err(|e| format!("failed to serialize ports: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(node_port: Option<u16>) -> PortMapping {
        PortMapping {
            container_port: 80,
            protocol: None,
            rg_port: None,
            node_port,
        }
    }

    #[test]
    fn explicit_port_already_used_is_rejected() {
        let used = HashSet::from([80]);
        let error = ensure_requested_free(&[mapping(Some(80))], &used).unwrap_err();
        assert!(error.contains("80"));
    }

    #[test]
    fn free_and_unset_ports_are_accepted() {
        let used = HashSet::from([80]);
        assert!(ensure_requested_free(&[mapping(Some(443)), mapping(None)], &used).is_ok());
    }
}
