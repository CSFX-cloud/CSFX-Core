use anyhow::Result;
use chrono::Utc;
use entity::entities::failover_events;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::models::event::FailoverEvent;

pub async fn insert(
    db: &DatabaseConnection,
    agent_id: Option<Uuid>,
    event_type: &str,
    affected_workloads: Option<Vec<Uuid>>,
    duration_ms: Option<i64>,
) -> Result<failover_events::Model> {
    insert_with_severity(
        db,
        agent_id,
        event_type,
        "warning",
        None,
        affected_workloads,
        duration_ms,
    )
    .await
}

pub async fn insert_with_severity(
    db: &DatabaseConnection,
    agent_id: Option<Uuid>,
    event_type: &str,
    severity: &str,
    message: Option<String>,
    affected_workloads: Option<Vec<Uuid>>,
    duration_ms: Option<i64>,
) -> Result<failover_events::Model> {
    let workloads_json =
        affected_workloads.map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let model = failover_events::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        event_type: Set(event_type.to_string()),
        affected_workloads: Set(workloads_json),
        duration_ms: Set(duration_ms),
        created_at: Set(Utc::now().naive_utc()),
        severity: Set(severity.to_string()),
        status: Set("open".to_string()),
        resolved_at: Set(None),
        message: Set(message),
    };

    Ok(model.insert(db).await?)
}

pub async fn find_open(
    db: &DatabaseConnection,
    agent_id: Uuid,
    event_type: &str,
) -> Result<Option<failover_events::Model>> {
    let row = failover_events::Entity::find()
        .filter(failover_events::Column::AgentId.eq(agent_id))
        .filter(failover_events::Column::EventType.eq(event_type))
        .filter(failover_events::Column::Status.eq("open"))
        .order_by_desc(failover_events::Column::CreatedAt)
        .one(db)
        .await?;

    Ok(row)
}

pub async fn resolve(db: &DatabaseConnection, event_id: Uuid) -> Result<()> {
    let event = failover_events::Entity::find_by_id(event_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Event not found event_id={}", event_id))?;

    let mut active: failover_events::ActiveModel = event.into();
    active.status = Set("resolved".to_string());
    active.resolved_at = Set(Some(Utc::now().naive_utc()));
    active.update(db).await?;

    Ok(())
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
        severity: m.severity,
        status: m.status,
        resolved_at: m.resolved_at.map(|t| t.and_utc()),
        message: m.message,
    }
}
