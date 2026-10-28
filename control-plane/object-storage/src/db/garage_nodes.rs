use anyhow::{Context, Result};
use chrono::Utc;
use entity::entities::garage_nodes;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use uuid::Uuid;

pub async fn list(db: &DatabaseConnection) -> Result<Vec<garage_nodes::Model>> {
    garage_nodes::Entity::find()
        .all(db)
        .await
        .context("failed to list garage nodes")
}

pub async fn mark_down(db: &DatabaseConnection, id: Uuid) -> Result<()> {
    if let Some(existing) = garage_nodes::Entity::find_by_id(id).one(db).await? {
        let mut model: garage_nodes::ActiveModel = existing.into();
        model.status = Set("down".to_string());
        model.updated_at = Set(Some(Utc::now().naive_utc()));
        model
            .update(db)
            .await
            .context("failed to mark garage node down")?;
    }
    Ok(())
}

pub async fn upsert_self(
    db: &DatabaseConnection,
    garage_node_id: &str,
    agent_id: Option<Uuid>,
    zone: &str,
    capacity_bytes: Option<i64>,
) -> Result<garage_nodes::Model> {
    let existing = garage_nodes::Entity::find()
        .filter(garage_nodes::Column::GarageNodeId.eq(garage_node_id))
        .one(db)
        .await
        .context("failed to look up existing garage node")?;

    let now = Utc::now().naive_utc();

    if let Some(existing) = existing {
        let mut model: garage_nodes::ActiveModel = existing.into();
        model.status = Set("up".to_string());
        model.agent_id = Set(agent_id);
        model.capacity_bytes = Set(capacity_bytes);
        model.role = Set(if capacity_bytes.is_some() {
            "storage".to_string()
        } else {
            "gateway".to_string()
        });
        model.last_seen_at = Set(Some(now));
        model.updated_at = Set(Some(now));
        return model
            .update(db)
            .await
            .context("failed to update self garage node");
    }

    let model = garage_nodes::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        garage_node_id: Set(Some(garage_node_id.to_string())),
        zone: Set(zone.to_string()),
        capacity_bytes: Set(capacity_bytes),
        role: Set(if capacity_bytes.is_some() {
            "storage".to_string()
        } else {
            "gateway".to_string()
        }),
        status: Set("up".to_string()),
        layout_version: Set(None),
        last_seen_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(None),
    };

    model
        .insert(db)
        .await
        .context("failed to insert self garage node")
}
