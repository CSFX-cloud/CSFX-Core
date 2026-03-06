use anyhow::Result;
use chrono::Utc;
use entity::entities::agents;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub async fn set_agent_status(
    db: &DatabaseConnection,
    agent_id: Uuid,
    status: &str,
) -> Result<()> {
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

pub async fn get_stale_agents(
    db: &DatabaseConnection,
    soft_threshold_secs: i64,
    hard_threshold_secs: i64,
) -> Result<(Vec<agents::Model>, Vec<agents::Model>)> {
    let now = Utc::now().naive_utc();
    let soft_cutoff = now - chrono::Duration::seconds(soft_threshold_secs);
    let hard_cutoff = now - chrono::Duration::seconds(hard_threshold_secs);

    let all = agents::Entity::find()
        .filter(
            agents::Column::Status
                .eq("Online")
                .or(agents::Column::Status.eq("Degraded")),
        )
        .all(db)
        .await?;

    let mut degraded = Vec::new();
    let mut offline = Vec::new();

    for agent in all {
        let last_hb = match agent.last_heartbeat {
            Some(ts) => ts,
            None => continue,
        };

        if last_hb < hard_cutoff && agent.status == "Degraded" {
            offline.push(agent);
        } else if last_hb < soft_cutoff && agent.status == "Online" {
            degraded.push(agent);
        }
    }

    Ok((degraded, offline))
}
