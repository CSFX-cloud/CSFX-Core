use anyhow::Result;
use entity::agents;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QuerySelect, Set,
};
use uuid::Uuid;

use crate::models::agent::AgentStatistics;

pub struct NewAgent {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub agent_version: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub tags: Option<serde_json::Value>,
    pub wg_tunnel_ip: Option<String>,
    pub role: String,
    pub machine_id: Option<String>,
}

pub async fn get_by_machine_id(
    db: &DatabaseConnection,
    machine_id: &str,
) -> Result<Option<agents::Model>> {
    Ok(agents::Entity::find()
        .filter(agents::Column::MachineId.eq(machine_id))
        .one(db)
        .await?)
}

pub async fn name_taken(db: &DatabaseConnection, name: &str) -> Result<bool> {
    Ok(agents::Entity::find()
        .filter(agents::Column::Name.eq(name))
        .one(db)
        .await?
        .is_some())
}

pub async fn update_registration(
    db: &DatabaseConnection,
    agent_id: Uuid,
    agent_version: String,
    os_type: String,
    os_version: String,
    architecture: String,
    tags: Option<serde_json::Value>,
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
    agent.registered_at = Set(chrono::Utc::now().naive_utc());
    agent.last_heartbeat = Set(Some(chrono::Utc::now().naive_utc()));
    agent.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
    if tags.is_some() {
        agent.tags = Set(tags);
    }

    Ok(agent.update(db).await?)
}

pub async fn create(db: &DatabaseConnection, agent: NewAgent) -> Result<agents::Model> {
    let now = chrono::Utc::now().naive_utc();
    let model = agents::ActiveModel {
        id: Set(agent.id),
        name: Set(agent.name),
        hostname: Set(agent.hostname),
        ip_address: Set(None),
        agent_version: Set(agent.agent_version),
        os_type: Set(agent.os_type),
        os_version: Set(agent.os_version),
        architecture: Set(agent.architecture),
        status: Set("Online".to_string()),
        last_heartbeat: Set(Some(now)),
        registered_at: Set(now),
        updated_at: Set(None),
        organization_id: Set(None),
        tags: Set(agent.tags),
        capabilities: Set(None),
        public_key_pem: Set(None),
        wg_public_key: Set(None),
        wg_endpoint: Set(None),
        wg_tunnel_ip: Set(agent.wg_tunnel_ip),
        kvm_capable: Set(false),
        ssh_host_key: Set(None),
        cordoned: Set(false),
        maintenance_until: Set(None),
        role: Set(agent.role),
        machine_id: Set(agent.machine_id),
        cpu_overcommit_ratio: NotSet,
        memory_overcommit_ratio: NotSet,
        disk_overcommit_ratio: NotSet,
        ..Default::default()
    };

    Ok(model.insert(db).await?)
}

pub async fn get_by_id(db: &DatabaseConnection, agent_id: Uuid) -> Result<Option<agents::Model>> {
    Ok(agents::Entity::find_by_id(agent_id).one(db).await?)
}

pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<agents::Model>> {
    Ok(agents::Entity::find().all(db).await?)
}

pub async fn wg_endpoint_in_use(
    db: &DatabaseConnection,
    agent_id: Uuid,
    wg_endpoint: &str,
) -> Result<bool> {
    Ok(agents::Entity::find()
        .filter(agents::Column::WgEndpoint.eq(wg_endpoint))
        .filter(agents::Column::Id.ne(agent_id))
        .filter(agents::Column::Status.eq("Online"))
        .one(db)
        .await?
        .is_some())
}

pub async fn update_heartbeat(
    db: &DatabaseConnection,
    agent_id: Uuid,
    status: String,
    wg_public_key: Option<String>,
    wg_endpoint: Option<String>,
    agent_version: Option<String>,
    kvm_capable: bool,
    ssh_host_key: Option<String>,
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
    if let Some(wg_endpoint) = wg_endpoint {
        if let Some((host, _port)) = wg_endpoint.rsplit_once(':') {
            agent.ip_address = Set(Some(host.to_string()));
        }
        agent.wg_endpoint = Set(Some(wg_endpoint));
    }
    if let Some(agent_version) = agent_version {
        agent.agent_version = Set(agent_version);
    }
    agent.kvm_capable = Set(kvm_capable);
    if ssh_host_key.is_some() {
        agent.ssh_host_key = Set(ssh_host_key);
    }
    Ok(agent.update(db).await?)
}

pub async fn get_statistics(db: &DatabaseConnection) -> Result<AgentStatistics> {
    let counts: Vec<(String, i64)> = agents::Entity::find()
        .select_only()
        .column(agents::Column::Status)
        .column_as(agents::Column::Id.count(), "agents")
        .group_by(agents::Column::Status)
        .into_tuple()
        .all(db)
        .await?;
    let count_of = |status: &str| {
        counts
            .iter()
            .find(|(name, _)| name == status)
            .map_or(0, |(_, count)| *count as usize)
    };

    Ok(AgentStatistics {
        total: counts.iter().map(|(_, count)| *count as usize).sum(),
        online: count_of("Online"),
        offline: count_of("Offline"),
        degraded: count_of("Degraded"),
    })
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
    let updated = agents::Entity::update_many()
        .col_expr(agents::Column::WgTunnelIp, Expr::value(wg_tunnel_ip))
        .filter(agents::Column::Id.eq(agent_id))
        .exec(db)
        .await?;
    anyhow::ensure!(updated.rows_affected == 1, "Agent not found");
    Ok(())
}
