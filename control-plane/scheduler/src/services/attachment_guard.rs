use std::collections::HashSet;

use entity::entities::{buckets, volumes};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
};
use uuid::Uuid;

use crate::models::workload::{BucketBinding, VolumeMount};

#[derive(Debug)]
pub enum AttachmentError {
    Forbidden,
    Database(DbErr),
}

impl From<DbErr> for AttachmentError {
    fn from(error: DbErr) -> Self {
        Self::Database(error)
    }
}

pub async fn ensure_same_resource_group(
    db: &DatabaseConnection,
    resource_group_id: Option<Uuid>,
    mounts: &[VolumeMount],
    bindings: &[BucketBinding],
) -> Result<(), AttachmentError> {
    let volume_ids: HashSet<Uuid> = mounts.iter().map(|mount| mount.volume_id).collect();
    let bucket_ids: HashSet<Uuid> = bindings.iter().map(|binding| binding.bucket_id).collect();

    let owned_volumes = count_volumes(db, resource_group_id, &volume_ids).await?;
    let owned_buckets = count_buckets(db, resource_group_id, &bucket_ids).await?;
    if owned_volumes != volume_ids.len() as u64 || owned_buckets != bucket_ids.len() as u64 {
        return Err(AttachmentError::Forbidden);
    }
    Ok(())
}

async fn count_volumes(
    db: &DatabaseConnection,
    resource_group_id: Option<Uuid>,
    ids: &HashSet<Uuid>,
) -> Result<u64, DbErr> {
    if ids.is_empty() {
        return Ok(0);
    }
    volumes::Entity::find()
        .filter(volumes::Column::Id.is_in(ids.iter().copied()))
        .filter(match resource_group_id {
            Some(id) => Condition::all().add(volumes::Column::ResourceGroupId.eq(id)),
            None => Condition::all().add(volumes::Column::ResourceGroupId.is_null()),
        })
        .count(db)
        .await
}

async fn count_buckets(
    db: &DatabaseConnection,
    resource_group_id: Option<Uuid>,
    ids: &HashSet<Uuid>,
) -> Result<u64, DbErr> {
    if ids.is_empty() {
        return Ok(0);
    }
    buckets::Entity::find()
        .filter(buckets::Column::Id.is_in(ids.iter().copied()))
        .filter(match resource_group_id {
            Some(id) => Condition::all().add(buckets::Column::ResourceGroupId.eq(id)),
            None => Condition::all().add(buckets::Column::ResourceGroupId.is_null()),
        })
        .count(db)
        .await
}
