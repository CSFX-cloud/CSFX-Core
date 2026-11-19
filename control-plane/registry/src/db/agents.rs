use anyhow::Result;
use entity::agents;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QuerySelect, Set,
};
use uuid::Uuid;

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

pub struct Registration {
    pub agent_version: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub tags: Option<serde_json::Value>,
}

pub struct NewAgent {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub registration: Registration,
    pub wg_tunnel_ip: Option<String>,
    pub role: String,
    pub machine_id: Option<String>,
}

pub struct Heartbeat {
    pub wg_public_key: Option<String>,
    pub wg_endpoint: Option<String>,
    pub agent_version: Option<String>,
    pub kvm_capable: bool,
    pub ssh_host_key: Option<String>,
}

async fn patch(
    db: &DatabaseConnection,
    agent_id: Uuid,
    mut changes: agents::ActiveModel,
) -> Result<agents::Model> {
    changes.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
    agents::Entity::update_many()
        .set(changes)
        .filter(agents::Column::Id.eq(agent_id))
        .exec_with_returning(db)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))
}

fn set_if_some<T: Into<sea_orm::Value> + sea_orm::sea_query::Nullable>(
    value: Option<T>,
) -> sea_orm::ActiveValue<Option<T>> {
    value.map_or(NotSet, |value| Set(Some(value)))
}

pub async fn update_registration(
    db: &DatabaseConnection,
    agent_id: Uuid,
    registration: Registration,
) -> Result<agents::Model> {
    let now = chrono::Utc::now().naive_utc();
    patch(
        db,
        agent_id,
        agents::ActiveModel {
            agent_version: Set(registration.agent_version),
            os_type: Set(registration.os_type),
            os_version: Set(registration.os_version),
            architecture: Set(registration.architecture),
            status: Set("Online".to_string()),
            registered_at: Set(now),
            last_heartbeat: Set(Some(now)),
            tags: registration.tags.map_or(NotSet, |tags| Set(Some(tags))),
            ..Default::default()
        },
    )
    .await
}

pub async fn create(db: &DatabaseConnection, agent: NewAgent) -> Result<agents::Model> {
    let now = chrono::Utc::now().naive_utc();
    let model = agents::ActiveModel {
        id: Set(agent.id),
        name: Set(agent.name),
        hostname: Set(agent.hostname),
        ip_address: Set(None),
        agent_version: Set(agent.registration.agent_version),
        os_type: Set(agent.registration.os_type),
        os_version: Set(agent.registration.os_version),
        architecture: Set(agent.registration.architecture),
        status: Set("Online".to_string()),
        last_heartbeat: Set(Some(now)),
        registered_at: Set(now),
        updated_at: Set(None),
        organization_id: Set(None),
        tags: Set(agent.registration.tags),
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
    heartbeat: Heartbeat,
) -> Result<agents::Model> {
    let reported_host = heartbeat
        .wg_endpoint
        .as_deref()
        .and_then(|endpoint| endpoint.rsplit_once(':'))
        .map(|(host, _port)| host.to_string());
    patch(
        db,
        agent_id,
        agents::ActiveModel {
            last_heartbeat: Set(Some(chrono::Utc::now().naive_utc())),
            status: Set("Online".to_string()),
            wg_public_key: set_if_some(heartbeat.wg_public_key),
            wg_endpoint: set_if_some(heartbeat.wg_endpoint),
            ip_address: set_if_some(reported_host),
            agent_version: heartbeat.agent_version.map_or(NotSet, Set),
            kvm_capable: Set(heartbeat.kvm_capable),
            ssh_host_key: set_if_some(heartbeat.ssh_host_key),
            ..Default::default()
        },
    )
    .await
}

pub async fn count_by_status(db: &DatabaseConnection) -> Result<Vec<(String, i64)>> {
    Ok(agents::Entity::find()
        .select_only()
        .column(agents::Column::Status)
        .column_as(agents::Column::Id.count(), "total")
        .group_by(agents::Column::Status)
        .into_tuple()
        .all(db)
        .await?)
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
    patch(
        db,
        agent_id,
        agents::ActiveModel {
            wg_tunnel_ip: Set(Some(wg_tunnel_ip.to_string())),
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}
