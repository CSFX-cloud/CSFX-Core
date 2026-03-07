use anyhow::Result;
use entity::agents;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn create(
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
    public_key_pem: Option<String>,
) -> Result<agents::Model> {
    let model = agents::ActiveModel {
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
        public_key_pem: Set(public_key_pem),
    };

    Ok(model.insert(db).await?)
}

pub async fn get_by_id(db: &DatabaseConnection, agent_id: Uuid) -> Result<Option<agents::Model>> {
    Ok(agents::Entity::find_by_id(agent_id).one(db).await?)
}

pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<agents::Model>> {
    Ok(agents::Entity::find().all(db).await?)
}

pub async fn update_heartbeat(
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

pub async fn mark_offline_by_timeout(
    db: &DatabaseConnection,
    timeout_seconds: i64,
) -> Result<u64> {
    let threshold =
        chrono::Utc::now().naive_utc() - chrono::Duration::seconds(timeout_seconds);

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

pub async fn get_statistics(db: &DatabaseConnection) -> Result<(usize, usize, usize, usize)> {
    let all = get_all(db).await?;
    let total = all.len();
    let online = all.iter().filter(|a| a.status == "Online").count();
    let offline = all.iter().filter(|a| a.status == "Offline").count();
    let degraded = all.iter().filter(|a| a.status == "Degraded").count();

    Ok((total, online, offline, degraded))
}

pub async fn delete(db: &DatabaseConnection, agent_id: Uuid) -> Result<()> {
    agents::Entity::delete_by_id(agent_id).exec(db).await?;
    Ok(())
}
