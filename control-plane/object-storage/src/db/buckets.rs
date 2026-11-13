use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use entity::entities::buckets;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use shared::pagination::{keyset_by_id, PageParams};
use uuid::Uuid;

use crate::models::{BucketResponse, CreateBucketRequest, UpdateBucketRequest};

pub fn into_response(model: buckets::Model) -> BucketResponse {
    BucketResponse {
        id: model.id,
        name: model.name,
        global_alias: model.global_alias,
        exposure: model.exposure,
        quota_max_size: model.quota_max_size,
        quota_max_objects: model.quota_max_objects,
        status: model.status,
        organization_id: model.organization_id,
        resource_group_id: model.resource_group_id,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

pub const STATUS_PENDING: &str = "pending";
const STATUS_ACTIVE: &str = "active";

pub async fn insert_pending(
    db: &DatabaseConnection,
    req: &CreateBucketRequest,
    global_alias: &str,
) -> Result<buckets::Model> {
    let model = buckets::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(req.name.clone()),
        garage_bucket_id: Set(None),
        global_alias: Set(global_alias.to_string()),
        exposure: Set(req
            .exposure
            .clone()
            .unwrap_or_else(|| "internal".to_string())),
        quota_max_size: Set(req.quota_max_size),
        quota_max_objects: Set(req.quota_max_objects),
        status: Set(STATUS_PENDING.to_string()),
        master_key_id: Set(None),
        master_key_secret_encrypted: Set(None),
        organization_id: Set(req.organization_id),
        resource_group_id: Set(req.resource_group_id),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    };

    model.insert(db).await.context("failed to insert bucket")
}

pub async fn record_garage_bucket(
    db: &DatabaseConnection,
    model: buckets::Model,
    garage_bucket_id: &str,
) -> Result<buckets::Model> {
    let mut active: buckets::ActiveModel = model.into();
    active.garage_bucket_id = Set(Some(garage_bucket_id.to_string()));
    active
        .update(db)
        .await
        .context("failed to record garage bucket")
}

pub async fn record_master_key(
    db: &DatabaseConnection,
    model: buckets::Model,
    master_key_id: &str,
) -> Result<buckets::Model> {
    let mut active: buckets::ActiveModel = model.into();
    active.master_key_id = Set(Some(master_key_id.to_string()));
    active
        .update(db)
        .await
        .context("failed to record master key")
}

pub async fn activate(
    db: &DatabaseConnection,
    model: buckets::Model,
    master_key_secret_encrypted: Vec<u8>,
) -> Result<buckets::Model> {
    let mut active: buckets::ActiveModel = model.into();
    active.master_key_secret_encrypted = Set(Some(master_key_secret_encrypted));
    active.status = Set(STATUS_ACTIVE.to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    active.update(db).await.context("failed to activate bucket")
}

pub async fn list_pending_before(
    db: &DatabaseConnection,
    cutoff: NaiveDateTime,
) -> Result<Vec<buckets::Model>> {
    buckets::Entity::find()
        .filter(buckets::Column::Status.eq(STATUS_PENDING))
        .filter(buckets::Column::CreatedAt.lt(cutoff))
        .all(db)
        .await
        .context("failed to list pending buckets")
}

pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<buckets::Model>> {
    buckets::Entity::find_by_id(id)
        .one(db)
        .await
        .context("failed to get bucket")
}

pub async fn list(
    db: &DatabaseConnection,
    resource_group_id: Option<Uuid>,
    organization_id: Option<Uuid>,
    after: Option<Uuid>,
    params: &PageParams,
) -> Result<Vec<buckets::Model>> {
    let mut query = buckets::Entity::find();

    if let Some(rg_id) = resource_group_id {
        query = query.filter(buckets::Column::ResourceGroupId.eq(rg_id));
    }

    if let Some(org_id) = organization_id {
        query = query.filter(buckets::Column::OrganizationId.eq(org_id));
    }

    keyset_by_id(query, buckets::Column::Id, after, params)
        .all(db)
        .await
        .context("failed to list buckets")
}

pub async fn update(
    db: &DatabaseConnection,
    id: Uuid,
    req: &UpdateBucketRequest,
) -> Result<Option<buckets::Model>> {
    let Some(existing) = get_by_id(db, id).await? else {
        return Ok(None);
    };

    let mut model: buckets::ActiveModel = existing.into();

    if let Some(exposure) = &req.exposure {
        model.exposure = Set(exposure.clone());
    }
    if req.quota_max_size.is_some() {
        model.quota_max_size = Set(req.quota_max_size);
    }
    if req.quota_max_objects.is_some() {
        model.quota_max_objects = Set(req.quota_max_objects);
    }
    model.updated_at = Set(Some(Utc::now().naive_utc()));

    let updated = model.update(db).await.context("failed to update bucket")?;
    Ok(Some(updated))
}

pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
    buckets::Entity::delete_by_id(id)
        .exec(db)
        .await
        .context("failed to delete bucket")?;
    Ok(())
}
