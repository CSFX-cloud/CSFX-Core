use anyhow::Result;
use entity::agents;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

pub async fn get_by_hostname(
    db: &DatabaseConnection,
    hostname: &str,
) -> Result<Option<agents::Model>> {
    Ok(agents::Entity::find()
        .filter(agents::Column::Hostname.eq(hostname))
        .one(db)
        .await?)
}

pub async fn update_registration(
    db: &DatabaseConnection,
    agent_id: Uuid,
    agent_version: String,
    os_type: String,
    os_version: String,
    architecture: String,
    tags: Option<serde_json::Value>,
    public_key_pem: Option<String>,
) -> Result<agents::Model> {
    let mut agent: agents::ActiveModel = agents::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?
        .into();

    agent.agent_version = Set(agent_version);
    agent.os_type = Set(os_type);
    agent.os_version = Set(os_version);
    agent.architecture = Set(architecture);
    agent.status = Set("Online".to_string());
    agent.last_heartbeat = Set(Some(chrono::Utc::now().naive_utc()));
    agent.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
    if tags.is_some() {
        agent.tags = Set(tags);
    }
    if public_key_pem.is_some() {
        agent.public_key_pem = Set(public_key_pem);
    }

    Ok(agent.update(db).await?)
}

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
    wg_tunnel_ip: Option<String>,
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
        wg_public_key: Set(None),
        wg_endpoint: Set(None),
        wg_tunnel_ip: Set(wg_tunnel_ip),
        kvm_capable: Set(false),
        cordoned: Set(false),
        maintenance_until: Set(None),
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
    wg_public_key: Option<String>,
    wg_endpoint: Option<String>,
    wg_tunnel_ip: Option<String>,
    agent_version: Option<String>,
    kvm_capable: bool,
) -> Result<agents::Model> {
    let mut agent: agents::ActiveModel = agents::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?
        .into();

    agent.last_heartbeat = Set(Some(chrono::Utc::now().naive_utc()));
    agent.status = Set(status);
    agent.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
    if wg_public_key.is_some() {
        agent.wg_public_key = Set(wg_public_key);
    }
    if wg_endpoint.is_some() {
        agent.wg_endpoint = Set(wg_endpoint);
    }
    if wg_tunnel_ip.is_some() {
        agent.wg_tunnel_ip = Set(wg_tunnel_ip);
    }
    if let Some(agent_version) = agent_version {
        agent.agent_version = Set(agent_version);
    }
    agent.kvm_capable = Set(kvm_capable);
    Ok(agent.update(db).await?)
}

pub async fn mark_degraded_by_timeout(
    db: &DatabaseConnection,
    timeout_seconds: i64,
) -> Result<u64> {
    let threshold = chrono::Utc::now().naive_utc() - chrono::Duration::seconds(timeout_seconds);
    let result = agents::Entity::update_many()
        .col_expr(
            agents::Column::Status,
            sea_orm::sea_query::Expr::value("Degraded"),
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

pub async fn mark_offline_by_timeout(db: &DatabaseConnection, timeout_seconds: i64) -> Result<u64> {
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
        .filter(
            agents::Column::Status
                .eq("Online")
                .or(agents::Column::Status.eq("Degraded")),
        )
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

pub async fn set_wg_tunnel_ip(
    db: &DatabaseConnection,
    agent_id: Uuid,
    wg_tunnel_ip: &str,
) -> Result<()> {
    let mut agent: agents::ActiveModel = agents::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?
        .into();

    agent.wg_tunnel_ip = Set(Some(wg_tunnel_ip.to_string()));
    agent.update(db).await?;
    Ok(())
}
