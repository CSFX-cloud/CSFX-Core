use anyhow::Result;
use entity::{agent_api_keys, agents, registry_tokens};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub async fn create_registry_token(
    db: &DatabaseConnection,
    token: String,
    description: Option<String>,
    created_by: String,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<registry_tokens::Model> {
    let token_model = registry_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        token: Set(token),
        description: Set(description),
        created_by: Set(created_by),
        created_at: Set(chrono::Utc::now().naive_utc()),
        expires_at: Set(expires_at.naive_utc()),
        used_at: Set(None),
        used_by_agent_id: Set(None),
        is_used: Set(false),
    };

    Ok(token_model.insert(db).await?)
}

pub async fn get_registry_token_by_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<registry_tokens::Model>> {
    Ok(registry_tokens::Entity::find()
        .filter(registry_tokens::Column::Token.eq(token))
        .one(db)
        .await?)
}

pub async fn mark_token_as_used(
    db: &DatabaseConnection,
    token_id: Uuid,
    agent_id: Uuid,
) -> Result<()> {
    let mut token: registry_tokens::ActiveModel =
        registry_tokens::Entity::find_by_id(token_id)
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

pub async fn get_unused_tokens(
    db: &DatabaseConnection,
) -> Result<Vec<registry_tokens::Model>> {
    Ok(registry_tokens::Entity::find()
        .filter(registry_tokens::Column::IsUsed.eq(false))
        .all(db)
        .await?)
}

pub async fn delete_expired_tokens(db: &DatabaseConnection) -> Result<u64> {
    let now = chrono::Utc::now().naive_utc();
    let result = registry_tokens::Entity::delete_many()
        .filter(registry_tokens::Column::ExpiresAt.lt(now))
        .filter(registry_tokens::Column::IsUsed.eq(false))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}

pub async fn create_agent(
    db: &DatabaseConnection,
    id: Uuid,
    name: String,
    hostname: String,
    ip_address: Option<String>,
    agent_version: String,
    os_type: String,
    os_version: String,
    architecture: String,
    status: String,
    tags: Option<serde_json::Value>,
    capabilities: Option<serde_json::Value>,
) -> Result<agents::Model> {
    let agent_model = agents::ActiveModel {
        id: Set(id),
        name: Set(name),
        hostname: Set(hostname),
        ip_address: Set(ip_address),
        agent_version: Set(agent_version),
        os_type: Set(os_type),
        os_version: Set(os_version),
        architecture: Set(architecture),
        status: Set(status),
        last_heartbeat: Set(Some(chrono::Utc::now().naive_utc())),
        registered_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(None),
        organization_id: Set(None),
        tags: Set(tags),
        capabilities: Set(capabilities),
    };

    Ok(agent_model.insert(db).await?)
}

pub async fn get_agent_by_id(
    db: &DatabaseConnection,
    agent_id: Uuid,
) -> Result<Option<agents::Model>> {
    Ok(agents::Entity::find_by_id(agent_id).one(db).await?)
}

pub async fn get_all_agents(db: &DatabaseConnection) -> Result<Vec<agents::Model>> {
    Ok(agents::Entity::find().all(db).await?)
}

pub async fn get_healthy_nodes(db: &DatabaseConnection) -> Result<Vec<agents::Model>> {
    Ok(agents::Entity::find()
        .filter(agents::Column::Status.eq("Online"))
        .all(db)
        .await?)
}

pub async fn update_agent_heartbeat(
    db: &DatabaseConnection,
    agent_id: Uuid,
    status: String,
) -> Result<()> {
    let mut agent: agents::ActiveModel = agents::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?
        .into();

    agent.last_heartbeat = Set(Some(chrono::Utc::now().naive_utc()));
    agent.status = Set(status);
    agent.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
    agent.update(db).await?;

    Ok(())
}

pub async fn update_agents_offline(
    db: &DatabaseConnection,
    timeout_seconds: i64,
) -> Result<u64> {
    let threshold = chrono::Utc::now().naive_utc() - chrono::Duration::seconds(timeout_seconds);

    let result = agents::Entity::update_many()
        .col_expr(
            agents::Column::Status,
            sea_orm::sea_query::Expr::value("Offline"),
        )
        .col_expr(
            agents::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(chrono::Utc::now().naive_utc()),
        )
        .filter(agents::Column::LastHeartbeat.lt(threshold))
        .filter(agents::Column::Status.eq("Online"))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}

pub async fn get_agent_statistics(
    db: &DatabaseConnection,
) -> Result<(usize, usize, usize, usize)> {
    let all_agents = get_all_agents(db).await?;
    let total = all_agents.len();
    let online = all_agents
        .iter()
        .filter(|a| a.status == "Online")
        .count();
    let offline = all_agents
        .iter()
        .filter(|a| a.status == "Offline")
        .count();
    let degraded = all_agents
        .iter()
        .filter(|a| a.status == "Degraded")
        .count();

    Ok((total, online, offline, degraded))
}

pub async fn create_api_key(
    db: &DatabaseConnection,
    agent_id: Uuid,
    api_key: String,
) -> Result<agent_api_keys::Model> {
    let api_key_model = agent_api_keys::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        api_key: Set(api_key),
        created_at: Set(chrono::Utc::now().naive_utc()),
        last_used_at: Set(None),
        is_active: Set(true),
    };

    Ok(api_key_model.insert(db).await?)
}

pub async fn get_agent_by_api_key(
    db: &DatabaseConnection,
    api_key: &str,
) -> Result<Option<agents::Model>> {
    let key = agent_api_keys::Entity::find()
        .filter(agent_api_keys::Column::ApiKey.eq(api_key))
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
