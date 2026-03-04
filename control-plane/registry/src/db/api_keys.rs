use anyhow::Result;
use entity::{agent_api_keys, agents};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub async fn create(
    db: &DatabaseConnection,
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

pub async fn get_agent_by_key(
    db: &DatabaseConnection,
    api_key: &str,
) -> Result<Option<agents::Model>> {
    let key_hash = hash_key(api_key);

    let key = agent_api_keys::Entity::find()
        .filter(agent_api_keys::Column::KeyHash.eq(key_hash))
        .filter(agent_api_keys::Column::IsActive.eq(true))
        .one(db)
        .await?;

    if let Some(key) = key {
        let mut key_active: agent_api_keys::ActiveModel = key.clone().into();
        key_active.last_used_at = Set(Some(chrono::Utc::now().naive_utc()));
        key_active.update(db).await?;

        Ok(agents::Entity::find_by_id(key.agent_id).one(db).await?)
    } else {
        Ok(None)
    }
}

pub async fn revoke_by_agent(db: &DatabaseConnection, agent_id: Uuid) -> Result<u64> {
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
