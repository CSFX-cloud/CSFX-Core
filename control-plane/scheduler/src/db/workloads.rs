use chrono::Utc;
use entity::entities::workloads;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, ModelTrait,
};
use uuid::Uuid;

use crate::models::workload::{CreateWorkloadRequest, WorkloadResponse, WorkloadStatus};

pub async fn create(
    db: &DatabaseConnection,
    req: &CreateWorkloadRequest,
) -> Result<workloads::Model, sea_orm::DbErr> {
    let env_vars = req
        .env_vars
        .as_ref()
        .and_then(|e| serde_json::to_value(e).ok());
    let ports = req
        .ports
        .as_ref()
        .and_then(|p| serde_json::to_value(p).ok());

    let model = workloads::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(req.name.clone()),
        image: Set(req.image.clone()),
        cpu_millicores: Set(req.cpu_millicores),
        memory_bytes: Set(req.memory_bytes),
        disk_bytes: Set(req.disk_bytes),
        env_vars: Set(env_vars),
        ports: Set(ports),
        status: Set(WorkloadStatus::Pending.as_str().to_string()),
        assigned_agent_id: Set(None),
        container_id: Set(None),
        created_by: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    };

    model.insert(db).await
}

pub async fn assign(
    db: &DatabaseConnection,
    workload_id: Uuid,
    agent_id: Uuid,
) -> Result<workloads::Model, sea_orm::DbErr> {
    let workload = workloads::Entity::find_by_id(workload_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(workload_id.to_string()))?;

    let mut active: workloads::ActiveModel = workload.into();
    active.assigned_agent_id = Set(Some(agent_id));
    active.status = Set(WorkloadStatus::Scheduled.as_str().to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    active.update(db).await
}

pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<WorkloadResponse>, sea_orm::DbErr> {
    let rows = workloads::Entity::find().all(db).await?;
    Ok(rows.into_iter().map(into_response).collect())
}

pub async fn update_container_status(
    db: &DatabaseConnection,
    workload_id: Uuid,
    container_id: &str,
    status: &str,
) -> Result<(), sea_orm::DbErr> {
    let workload = workloads::Entity::find_by_id(workload_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(workload_id.to_string()))?;

    let mut active: workloads::ActiveModel = workload.into();
    active.container_id = Set(Some(container_id.to_string()));
    active.status = Set(status.to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    active.update(db).await?;
    Ok(())
}

pub async fn delete(
    db: &DatabaseConnection,
    workload_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    let workload = workloads::Entity::find_by_id(workload_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(workload_id.to_string()))?;

    workload.delete(db).await?;
    Ok(())
}

fn into_response(m: workloads::Model) -> WorkloadResponse {
    WorkloadResponse {
        id: m.id,
        name: m.name,
        image: m.image,
        cpu_millicores: m.cpu_millicores,
        memory_bytes: m.memory_bytes,
        disk_bytes: m.disk_bytes,
        status: WorkloadStatus::from_str(&m.status),
        assigned_agent_id: m.assigned_agent_id,
        container_id: m.container_id,
        created_at: m.created_at.and_utc(),
        updated_at: m.updated_at.map(|dt| dt.and_utc()),
    }
}
