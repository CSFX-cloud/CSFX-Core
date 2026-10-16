use anyhow::Result;
use chrono::Utc;
use entity::entities::{agents, volumes, workloads};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use uuid::Uuid;

pub async fn set_agent_status(db: &DatabaseConnection, agent_id: Uuid, status: &str) -> Result<()> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found agent_id={}", agent_id))?;

    let mut active: agents::ActiveModel = agent.into();
    active.status = Set(status.to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    active.update(db).await?;

    Ok(())
}

pub async fn set_maintenance(
    db: &DatabaseConnection,
    agent_id: Uuid,
    until: Option<chrono::NaiveDateTime>,
) -> Result<()> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found agent_id={}", agent_id))?;

    let mut active: agents::ActiveModel = agent.into();
    active.maintenance_until = Set(until);
    active.update(db).await?;

    Ok(())
}

pub async fn get_agents_for_alerting(db: &DatabaseConnection) -> Result<Vec<agents::Model>> {
    let now = Utc::now().naive_utc();

    let agents = agents::Entity::find()
        .filter(
            agents::Column::MaintenanceUntil
                .is_null()
                .or(agents::Column::MaintenanceUntil.lt(now)),
        )
        .all(db)
        .await?;

    Ok(agents)
}

pub async fn get_stale_agents(
    db: &DatabaseConnection,
    soft_threshold_secs: i64,
    hard_threshold_secs: i64,
) -> Result<(Vec<agents::Model>, Vec<agents::Model>)> {
    let now = Utc::now().naive_utc();
    let soft_cutoff = now - chrono::Duration::seconds(soft_threshold_secs);
    let hard_cutoff = now - chrono::Duration::seconds(hard_threshold_secs);

    let all = agents::Entity::find()
        .filter(agents::Column::Status.is_in(["Online", "Degraded", "Offline"]))
        .all(db)
        .await?;

    let mut degraded = Vec::new();
    let mut offline = Vec::new();

    for agent in all {
        if agent.maintenance_until.is_some_and(|until| until > now) {
            continue;
        }

        let last_hb = match agent.last_heartbeat {
            Some(ts) => ts,
            None => continue,
        };

        if last_hb < hard_cutoff && agent.status != "Online" {
            offline.push(agent);
        } else if last_hb < soft_cutoff && agent.status == "Online" {
            degraded.push(agent);
        }
    }

    Ok((degraded, offline))
}

pub async fn count_online(db: &DatabaseConnection) -> Result<u64> {
    Ok(agents::Entity::find()
        .filter(agents::Column::Status.eq("Online"))
        .count(db)
        .await?)
}

pub async fn has_active_assignments(db: &DatabaseConnection, agent_id: Uuid) -> Result<bool> {
    let workload_count = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.eq(agent_id))
        .filter(workloads::Column::Status.is_in(["scheduled", "running"]))
        .count(db)
        .await?;
    if workload_count > 0 {
        return Ok(true);
    }
    let volume_count = volumes::Entity::find()
        .filter(volumes::Column::AttachedToAgent.eq(agent_id))
        .count(db)
        .await?;
    Ok(volume_count > 0)
}
