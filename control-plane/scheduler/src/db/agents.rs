use entity::entities::{agent_metrics, agents, volumes, workloads};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::workload::AgentResources;
use shared::workload_status::ACTIVE_STATUSES;

const MILLICORES_PER_CORE: i32 = 1000;
const SYSTEM_RESERVE_FRACTION: f64 = 0.1;

pub async fn get_schedulable_agents(
    db: &DatabaseConnection,
) -> Result<Vec<AgentResources>, sea_orm::DbErr> {
    let mut agents = get_online_agents_with_capacity(db).await?;
    let mut reserved = get_reserved_by_agent(db).await?;

    for agent in agents.iter_mut() {
        let Some(reservation) = reserved.remove(&agent.agent_id) else {
            continue;
        };
        agent.free_cpu_millicores -= reservation.cpu_millicores;
        agent.free_memory_bytes -= reservation.memory_bytes;
        agent.free_disk_bytes -= reservation.disk_bytes;
        agent.spread_counts = reservation.spread_counts;
    }

    Ok(agents)
}

#[derive(Default)]
struct Reservation {
    cpu_millicores: i32,
    memory_bytes: i64,
    disk_bytes: i64,
    spread_counts: HashMap<String, u32>,
}

async fn get_reserved_by_agent(
    db: &DatabaseConnection,
) -> Result<HashMap<Uuid, Reservation>, sea_orm::DbErr> {
    let active = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(db)
        .await?;

    let mut reserved: HashMap<Uuid, Reservation> = HashMap::new();
    for workload in active {
        let Some(agent_id) = workload.assigned_agent_id else {
            continue;
        };
        let entry = reserved.entry(agent_id).or_default();
        entry.cpu_millicores += workload.cpu_millicores;
        entry.memory_bytes += workload.memory_bytes;
        entry.disk_bytes += workload.disk_bytes;
        if let Some(key) = workload.spread_key {
            *entry.spread_counts.entry(key).or_default() += 1;
        }
    }

    Ok(reserved)
}

fn allocatable(total: f64, ratio: f64) -> f64 {
    total * (1.0 - SYSTEM_RESERVE_FRACTION) * ratio
}

async fn get_online_agents_with_capacity(
    db: &DatabaseConnection,
) -> Result<Vec<AgentResources>, sea_orm::DbErr> {
    let online_agents = agents::Entity::find()
        .filter(agents::Column::Status.eq("Online"))
        .filter(agents::Column::Cordoned.eq(false))
        .all(db)
        .await?;

    let mut result = Vec::with_capacity(online_agents.len());

    for agent in online_agents {
        let latest_metrics = agent_metrics::Entity::find()
            .filter(agent_metrics::Column::AgentId.eq(agent.id))
            .order_by_desc(agent_metrics::Column::Timestamp)
            .one(db)
            .await?;

        let Some(m) = latest_metrics else {
            continue;
        };

        let cpu_millicores = allocatable(
            (m.cpu_cores.unwrap_or(0) * MILLICORES_PER_CORE) as f64,
            agent.cpu_overcommit_ratio,
        ) as i32;
        let memory_bytes = allocatable(
            m.memory_total_bytes.unwrap_or(0) as f64,
            agent.memory_overcommit_ratio,
        ) as i64;
        let disk_bytes = allocatable(
            m.disk_total_bytes.unwrap_or(0) as f64,
            agent.disk_overcommit_ratio,
        ) as i64;

        result.push(AgentResources {
            agent_id: agent.id,
            allocatable_cpu_millicores: cpu_millicores,
            allocatable_memory_bytes: memory_bytes,
            free_cpu_millicores: cpu_millicores,
            free_memory_bytes: memory_bytes,
            free_disk_bytes: disk_bytes,
            kvm_capable: agent.kvm_capable,
            spread_counts: HashMap::new(),
        });
    }

    Ok(result)
}

pub async fn get_agents_hosting_resource_group(
    db: &DatabaseConnection,
    resource_group_id: Uuid,
) -> Result<std::collections::HashSet<Uuid>, sea_orm::DbErr> {
    let rows = workloads::Entity::find()
        .filter(workloads::Column::ResourceGroupId.eq(resource_group_id))
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(db)
        .await?;

    Ok(rows
        .into_iter()
        .filter_map(|w| w.assigned_agent_id)
        .collect())
}

pub async fn get_volume_agent(
    db: &DatabaseConnection,
    volume_id: Uuid,
) -> Result<Option<Uuid>, sea_orm::DbErr> {
    let vol = volumes::Entity::find_by_id(volume_id).one(db).await?;
    Ok(vol.and_then(|v| v.attached_to_agent))
}
