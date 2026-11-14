use anyhow::Result;
use entity::agent_api_keys;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

pub async fn create(
    db: &impl ConnectionTrait,
    agent_id: Uuid,
    key_hash: String,
) -> Result<agent_api_keys::Model> {
    let model = agent_api_keys::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        key_hash: Set(key_hash),
        created_at: Set(chrono::Utc::now().naive_utc()),
        last_used_at: Set(None),
        is_active: Set(true),
    };

    Ok(model.insert(db).await?)
}

pub async fn revoke_by_agent(db: &impl ConnectionTrait, agent_id: Uuid) -> Result<u64> {
    let result = agent_api_keys::Entity::update_many()
        .col_expr(
            agent_api_keys::Column::IsActive,
            sea_orm::sea_query::Expr::value(false),
        )
        .filter(agent_api_keys::Column::AgentId.eq(agent_id))
        .filter(agent_api_keys::Column::IsActive.eq(true))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}
