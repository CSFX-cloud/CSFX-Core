use anyhow::Result;
use entity::registry_tokens;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub async fn create(
    db: &DatabaseConnection,
    agent_id: Uuid,
    token: String,
    expected_name: String,
    expected_hostname: String,
    description: Option<String>,
    created_by: String,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<registry_tokens::Model> {
    let model = registry_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        token: Set(token),
        agent_id: Set(Some(agent_id)),
        expected_name: Set(expected_name),
        expected_hostname: Set(expected_hostname),
        description: Set(description),
        created_by: Set(created_by),
        created_at: Set(chrono::Utc::now().naive_utc()),
        expires_at: Set(expires_at.naive_utc()),
        used_at: Set(None),
        used_by_agent_id: Set(None),
        is_used: Set(false),
    };

    Ok(model.insert(db).await?)
}

pub async fn get_by_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<registry_tokens::Model>> {
    Ok(registry_tokens::Entity::find()
        .filter(registry_tokens::Column::Token.eq(token))
        .one(db)
        .await?)
}

pub async fn mark_used(db: &DatabaseConnection, token_id: Uuid, agent_id: Uuid) -> Result<()> {
    let mut token: registry_tokens::ActiveModel = registry_tokens::Entity::find_by_id(token_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Token not found"))?
        .into();

    token.is_used = Set(true);
    token.used_at = Set(Some(chrono::Utc::now().naive_utc()));
    token.used_by_agent_id = Set(Some(agent_id));
    token.update(db).await?;

    Ok(())
}

pub async fn get_unused(db: &DatabaseConnection) -> Result<Vec<registry_tokens::Model>> {
    Ok(registry_tokens::Entity::find()
        .filter(registry_tokens::Column::IsUsed.eq(false))
        .all(db)
        .await?)
}

pub async fn delete_expired(db: &DatabaseConnection) -> Result<u64> {
    let now = chrono::Utc::now().naive_utc();
    let result = registry_tokens::Entity::delete_many()
        .filter(registry_tokens::Column::ExpiresAt.lt(now))
        .filter(registry_tokens::Column::IsUsed.eq(false))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}

pub async fn delete_by_agent(db: &DatabaseConnection, agent_id: Uuid) -> Result<u64> {
    let result = registry_tokens::Entity::delete_many()
        .filter(registry_tokens::Column::AgentId.eq(agent_id))
        .filter(registry_tokens::Column::IsUsed.eq(false))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}
