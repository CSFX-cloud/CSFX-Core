use entity::entities::{agent_metrics, agents, volumes, workloads};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::workload::AgentResources;

const MILLICORES_PER_CORE: i32 = 1000;
const RESERVING_STATUSES: [&str; 5] = ["scheduled", "pulling", "creating", "starting", "running"];

pub async fn get_schedulable_agents(
    db: &DatabaseConnection,
) -> Result<Vec<AgentResources>, sea_orm::DbErr> {
    let mut agents = get_online_agents_with_capacity(db).await?;
    let reserved = get_reserved_by_agent(db).await?;

    for agent in agents.iter_mut() {
        let Some((cpu, memory, disk)) = reserved.get(&agent.agent_id) else {
            continue;
        };
        agent.free_cpu_millicores -= cpu;
        agent.free_memory_bytes -= memory;
        agent.free_disk_bytes -= disk;
    }

    Ok(agents)
}

async fn get_reserved_by_agent(
    db: &DatabaseConnection,
) -> Result<HashMap<Uuid, (i32, i64, i64)>, sea_orm::DbErr> {
    let active = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::Status.is_in(RESERVING_STATUSES))
        .all(db)
        .await?;

    let mut reserved: HashMap<Uuid, (i32, i64, i64)> = HashMap::new();
    for workload in active {
        let Some(agent_id) = workload.assigned_agent_id else {
            continue;
        };
        let entry = reserved.entry(agent_id).or_default();
        entry.0 += workload.cpu_millicores;
        entry.1 += workload.memory_bytes;
        entry.2 += workload.disk_bytes;
    }

    Ok(reserved)
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

        result.push(AgentResources {
            agent_id: agent.id,
            free_cpu_millicores: m.cpu_cores.unwrap_or(0) * MILLICORES_PER_CORE,
            free_memory_bytes: m.memory_total_bytes.unwrap_or(0),
            free_disk_bytes: m.disk_total_bytes.unwrap_or(0),
            kvm_capable: agent.kvm_capable,
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
        .filter(
            workloads::Column::Status
                .eq("scheduled")
                .or(workloads::Column::Status.eq("running")),
        )
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
