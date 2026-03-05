use entity::entities::{agent_metrics, agents};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::models::workload::AgentResources;


const TOTAL_CPU_MILLICORES: i32 = 4000;
const TOTAL_MEMORY_BYTES: i64 = 8 * 1024 * 1024 * 1024;
const TOTAL_DISK_BYTES: i64 = 100 * 1024 * 1024 * 1024;

pub async fn get_online_agents_with_resources(
    db: &DatabaseConnection,
) -> Result<Vec<AgentResources>, sea_orm::DbErr> {
    let online_agents = agents::Entity::find()
        .filter(agents::Column::Status.eq("Online"))
        .all(db)
        .await?;

    let mut result = Vec::with_capacity(online_agents.len());

    for agent in online_agents {
        let latest_metrics = agent_metrics::Entity::find()
            .filter(agent_metrics::Column::AgentId.eq(agent.id))
            .order_by_desc(agent_metrics::Column::Timestamp)
            .one(db)
            .await?;

        let (used_cpu, used_mem, used_disk) = match latest_metrics {
            Some(m) => {
                let cpu = m.cpu_usage_percent.unwrap_or(0.0);
                let used_cpu = ((cpu / 100.0) * TOTAL_CPU_MILLICORES as f32) as i32;
                let used_mem = m.memory_used_bytes.unwrap_or(0);
                let used_disk = m.disk_used_bytes.unwrap_or(0);
                (used_cpu, used_mem, used_disk)
            }
            None => (0, 0, 0),
        };

        result.push(AgentResources {
            agent_id: agent.id,
            free_cpu_millicores: TOTAL_CPU_MILLICORES - used_cpu,
            free_memory_bytes: TOTAL_MEMORY_BYTES - used_mem,
            free_disk_bytes: TOTAL_DISK_BYTES - used_disk,
        });
    }

    Ok(result)
}

pub async fn get_assigned_workload_resources(
    db: &DatabaseConnection,
    agent_id: Uuid,
) -> Result<(i32, i64, i64), sea_orm::DbErr> {
    use entity::entities::workloads;

    let workloads = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.eq(agent_id))
        .filter(
            workloads::Column::Status
                .eq("scheduled")
                .or(workloads::Column::Status.eq("running")),
        )
        .all(db)
        .await?;

    let cpu: i32 = workloads.iter().map(|w| w.cpu_millicores).sum();
    let mem: i64 = workloads.iter().map(|w| w.memory_bytes).sum();
    let disk: i64 = workloads.iter().map(|w| w.disk_bytes).sum();


    Ok((cpu, mem, disk))
}
