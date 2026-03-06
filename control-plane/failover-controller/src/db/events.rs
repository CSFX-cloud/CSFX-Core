use anyhow::Result;
use chrono::Utc;
use entity::entities::failover_events;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryOrder, Set};
use uuid::Uuid;

use crate::models::event::FailoverEvent;

pub async fn insert(
    db: &DatabaseConnection,
    agent_id: Option<Uuid>,
    event_type: &str,
    affected_workloads: Option<Vec<Uuid>>,
    duration_ms: Option<i64>,
) -> Result<failover_events::Model> {
    let workloads_json = affected_workloads
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let model = failover_events::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        event_type: Set(event_type.to_string()),
        affected_workloads: Set(workloads_json),
        duration_ms: Set(duration_ms),
        created_at: Set(Utc::now().naive_utc()),
    };

    Ok(model.insert(db).await?)
}

pub async fn get_recent(db: &DatabaseConnection, limit: u64) -> Result<Vec<FailoverEvent>> {
    let rows: Vec<failover_events::Model> = failover_events::Entity::find()
        .order_by_desc(failover_events::Column::CreatedAt)
        .paginate(db, limit)
        .fetch_page(0)
        .await?;

    Ok(rows.into_iter().map(into_event).collect())
}

fn into_event(m: failover_events::Model) -> FailoverEvent {
    FailoverEvent {
        id: m.id,
        agent_id: m.agent_id,
        event_type: m.event_type,
        affected_workloads: m
            .affected_workloads
            .and_then(|v| serde_json::from_value(v).ok()),
        duration_ms: m.duration_ms,
        created_at: m.created_at.and_utc(),
    }
}
