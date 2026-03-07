use anyhow::{Context, Result};
use chrono::Utc;
use entity::entities::{network_members, network_policies, networks};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use uuid::Uuid;

use crate::models::{CreateNetworkRequest, CreatePolicyRequest};

pub async fn create_network(
    db: &DatabaseConnection,
    req: CreateNetworkRequest,
) -> Result<networks::Model> {
    let model = networks::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(req.name),
        cidr: Set(req.cidr),
        overlay_type: Set(req.overlay_type),
        status: Set("active".to_string()),
        organization_id: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    };

    model
        .insert(db)
        .await
        .context("Failed to insert network")
}

pub async fn list_networks(db: &DatabaseConnection) -> Result<Vec<networks::Model>> {
    networks::Entity::find()
        .all(db)
        .await
        .context("Failed to list networks")
}

pub async fn get_network(db: &DatabaseConnection, id: Uuid) -> Result<Option<networks::Model>> {
    networks::Entity::find_by_id(id)
        .one(db)
        .await
        .context("Failed to get network")
}

pub async fn delete_network(db: &DatabaseConnection, id: Uuid) -> Result<()> {
    networks::Entity::delete_by_id(id)
        .exec(db)
        .await
        .context("Failed to delete network")?;
    Ok(())
}

pub async fn create_policy(
    db: &DatabaseConnection,
    network_id: Uuid,
    req: CreatePolicyRequest,
) -> Result<network_policies::Model> {
    let model = network_policies::ActiveModel {
        id: Set(Uuid::new_v4()),
        network_id: Set(network_id),
        direction: Set(req.direction),
        action: Set(req.action),
        source_cidr: Set(req.source_cidr),
        destination_cidr: Set(req.destination_cidr),
        port: Set(req.port),
        protocol: Set(req.protocol),
        priority: Set(req.priority),
        created_at: Set(Utc::now().naive_utc()),
    };

    model
        .insert(db)
        .await
        .context("Failed to insert network policy")
}

pub async fn list_policies(
    db: &DatabaseConnection,
    network_id: Uuid,
) -> Result<Vec<network_policies::Model>> {
    use entity::entities::network_policies::Column;
    network_policies::Entity::find()
        .filter(Column::NetworkId.eq(network_id))
        .all(db)
        .await
        .context("Failed to list network policies")
}

pub async fn add_member(
    db: &DatabaseConnection,
    network_id: Uuid,
    workload_id: Uuid,
    allocated_ip: String,
) -> Result<network_members::Model> {
    let model = network_members::ActiveModel {
        id: Set(Uuid::new_v4()),
        network_id: Set(network_id),
        workload_id: Set(workload_id),
        allocated_ip: Set(allocated_ip),
        created_at: Set(Utc::now().naive_utc()),
    };

    model
        .insert(db)
        .await
        .context("Failed to insert network member")
}

pub async fn list_members(
    db: &DatabaseConnection,
    network_id: Uuid,
) -> Result<Vec<network_members::Model>> {
    use entity::entities::network_members::Column;
    network_members::Entity::find()
        .filter(Column::NetworkId.eq(network_id))
        .all(db)
        .await
        .context("Failed to list network members")
}

pub async fn remove_member(
    db: &DatabaseConnection,
    network_id: Uuid,
    workload_id: Uuid,
) -> Result<()> {
    use entity::entities::network_members::Column;
    network_members::Entity::delete_many()
        .filter(Column::NetworkId.eq(network_id))
        .filter(Column::WorkloadId.eq(workload_id))
        .exec(db)
        .await
        .context("Failed to remove network member")?;
    Ok(())
}
