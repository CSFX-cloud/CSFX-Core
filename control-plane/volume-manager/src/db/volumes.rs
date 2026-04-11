use chrono::Utc;
use entity::entities::{volume_snapshots, volumes};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, ModelTrait,
    QueryFilter, ColumnTrait,
};
use uuid::Uuid;

use crate::models::volume::{
    CreateVolumeRequest, SnapshotResponse, SnapshotStatus, VolumeResponse, VolumeStatus,
};

pub async fn create(
    db: &DatabaseConnection,
    req: &CreateVolumeRequest,
) -> Result<volumes::Model, sea_orm::DbErr> {
    let pool = req.pool.clone().unwrap_or_else(|| "csfx-volumes".to_string());
    let image_name = format!("{}-{}", req.name, Uuid::new_v4());

    let model = volumes::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(req.name.clone()),
        size_gb: Set(req.size_gb),
        pool: Set(pool),
        image_name: Set(image_name),
        status: Set(VolumeStatus::Available.as_str().to_string()),
        attached_to_agent: Set(None),
        attached_to_workload: Set(None),
        mapped_device: Set(None),
        organization_id: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    };

    model.insert(db).await
}

pub async fn get_by_id(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<Option<volumes::Model>, sea_orm::DbErr> {
    volumes::Entity::find_by_id(id).one(db).await
}

pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<VolumeResponse>, sea_orm::DbErr> {
    let rows = volumes::Entity::find().all(db).await?;
    Ok(rows.into_iter().map(into_response).collect())
}

pub async fn attach(
    db: &DatabaseConnection,
    volume_id: Uuid,
    agent_id: Uuid,
    workload_id: Option<Uuid>,
    device: Option<String>,
) -> Result<volumes::Model, sea_orm::DbErr> {
    let volume = volumes::Entity::find_by_id(volume_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(volume_id.to_string()))?;

    let mut active: volumes::ActiveModel = volume.into();
    active.attached_to_agent = Set(Some(agent_id));
    active.attached_to_workload = Set(workload_id);
    active.mapped_device = Set(device);
    active.status = Set(VolumeStatus::InUse.as_str().to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    active.update(db).await
}

pub async fn detach(
    db: &DatabaseConnection,
    volume_id: Uuid,
) -> Result<volumes::Model, sea_orm::DbErr> {
    let volume = volumes::Entity::find_by_id(volume_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(volume_id.to_string()))?;

    let mut active: volumes::ActiveModel = volume.into();
    active.attached_to_agent = Set(None);
    active.attached_to_workload = Set(None);
    active.mapped_device = Set(None);
    active.status = Set(VolumeStatus::Available.as_str().to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    active.update(db).await
}

pub async fn delete(db: &DatabaseConnection, volume_id: Uuid) -> Result<(), sea_orm::DbErr> {
    let volume = volumes::Entity::find_by_id(volume_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(volume_id.to_string()))?;

    volume.delete(db).await?;
    Ok(())
}

pub async fn create_snapshot(
    db: &DatabaseConnection,
    volume_id: Uuid,
    name: &str,
) -> Result<volume_snapshots::Model, sea_orm::DbErr> {
    let model = volume_snapshots::ActiveModel {
        id: Set(Uuid::new_v4()),
        volume_id: Set(volume_id),
        name: Set(name.to_string()),
        status: Set(SnapshotStatus::Available.as_str().to_string()),
        created_at: Set(Utc::now().naive_utc()),
    };

    model.insert(db).await
}

pub async fn list_snapshots(
    db: &DatabaseConnection,
    volume_id: Uuid,
) -> Result<Vec<SnapshotResponse>, sea_orm::DbErr> {
    let rows = volume_snapshots::Entity::find()
        .filter(volume_snapshots::Column::VolumeId.eq(volume_id))
        .all(db)
        .await?;

    Ok(rows.into_iter().map(into_snapshot_response).collect())
}

pub fn into_response(m: volumes::Model) -> VolumeResponse {
    VolumeResponse {
        id: m.id,
        name: m.name,
        size_gb: m.size_gb,
        pool: m.pool,
        image_name: m.image_name,
        status: VolumeStatus::from_str(&m.status),
        attached_to_agent: m.attached_to_agent,
        attached_to_workload: m.attached_to_workload,
        mapped_device: m.mapped_device,
        created_at: m.created_at.and_utc(),
        updated_at: m.updated_at.map(|dt| dt.and_utc()),
    }
}

fn into_snapshot_response(m: volume_snapshots::Model) -> SnapshotResponse {
    SnapshotResponse {
        id: m.id,
        volume_id: m.volume_id,
        name: m.name,
        status: SnapshotStatus::Available,
        created_at: m.created_at.and_utc(),
    }
}
