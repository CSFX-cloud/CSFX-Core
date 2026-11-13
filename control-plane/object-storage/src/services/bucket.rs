use anyhow::{bail, Result};
use entity::entities::buckets;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    crypto::SecretBox,
    db::buckets as db,
    garage::{
        client::{GarageBucket, GarageKey},
        GarageClient,
    },
    log_error, log_info,
    models::{BucketResponse, CreateBucketRequest, UpdateBucketRequest},
};

pub async fn create_bucket(
    db_conn: &DatabaseConnection,
    garage: &GarageClient,
    secret_box: &SecretBox,
    req: CreateBucketRequest,
) -> Result<BucketResponse> {
    let global_alias = format!("{}-{}", req.name, Uuid::new_v4().simple());
    let pending = db::insert_pending(db_conn, &req, &global_alias).await?;
    let pending_id = pending.id;

    match provision(db_conn, garage, secret_box, &req, pending).await {
        Ok(model) => {
            log_info!(
                "services::bucket",
                &format!(
                    "bucket created id={} garage_bucket_id={}",
                    model.id,
                    model.garage_bucket_id.as_deref().unwrap_or_default()
                )
            );
            Ok(db::into_response(model))
        }
        Err(e) => {
            if let Err(cleanup) = db::delete(db_conn, pending_id).await {
                log_error!(
                    "services::bucket",
                    &format!(
                        "pending bucket row cleanup failed id={} err={}",
                        pending_id, cleanup
                    )
                );
            }
            Err(e)
        }
    }
}

async fn provision(
    db_conn: &DatabaseConnection,
    garage: &GarageClient,
    secret_box: &SecretBox,
    req: &CreateBucketRequest,
    pending: buckets::Model,
) -> Result<buckets::Model> {
    let garage_bucket = create_garage_bucket(garage, req, &pending.global_alias).await?;
    let pending = db::record_garage_bucket(db_conn, pending, &garage_bucket.id).await?;
    let master_key = create_master_key(garage, &garage_bucket.id, &pending.global_alias).await?;
    let pending = db::record_master_key(db_conn, pending, &master_key.access_key_id).await?;
    let encrypted_secret = secret_box.encrypt(&master_key.secret_access_key)?;
    db::activate(db_conn, pending, encrypted_secret).await
}

async fn create_garage_bucket(
    garage: &GarageClient,
    req: &CreateBucketRequest,
    global_alias: &str,
) -> Result<GarageBucket> {
    let garage_bucket = match garage.create_bucket(global_alias).await {
        Ok(bucket) => bucket,
        Err(e) => {
            log_error!(
                "services::bucket",
                &format!("garage create_bucket failed name={} err={}", req.name, e)
            );
            bail!("failed to create bucket in garage: {}", e);
        }
    };

    if req.quota_max_size.is_none() && req.quota_max_objects.is_none() {
        return Ok(garage_bucket);
    }
    if let Err(e) = garage
        .update_bucket_quotas(&garage_bucket.id, req.quota_max_size, req.quota_max_objects)
        .await
    {
        log_error!(
            "services::bucket",
            &format!(
                "garage update_bucket_quotas failed bucket_id={} err={}",
                garage_bucket.id, e
            )
        );
        let _ = garage.delete_bucket(&garage_bucket.id).await;
        bail!("failed to apply bucket quota: {}", e);
    }
    Ok(garage_bucket)
}

async fn create_master_key(
    garage: &GarageClient,
    garage_bucket_id: &str,
    global_alias: &str,
) -> Result<GarageKey> {
    let master_key = match garage.create_key(&format!("{}-master", global_alias)).await {
        Ok(key) => key,
        Err(e) => {
            log_error!(
                "services::bucket",
                &format!(
                    "garage create_key for master key failed bucket_id={} err={}",
                    garage_bucket_id, e
                )
            );
            let _ = garage.delete_bucket(garage_bucket_id).await;
            bail!("failed to create master access key: {}", e);
        }
    };

    if let Err(e) = garage
        .allow_bucket_key(garage_bucket_id, &master_key.access_key_id, "owner")
        .await
    {
        log_error!(
            "services::bucket",
            &format!(
                "garage allow_bucket_key for master key failed bucket_id={} err={}",
                garage_bucket_id, e
            )
        );
        let _ = garage.delete_key(&master_key.access_key_id).await;
        let _ = garage.delete_bucket(garage_bucket_id).await;
        bail!("failed to grant master key access: {}", e);
    }
    Ok(master_key)
}

pub async fn get_bucket(db_conn: &DatabaseConnection, id: Uuid) -> Result<Option<BucketResponse>> {
    Ok(db::get_by_id(db_conn, id).await?.map(db::into_response))
}

pub async fn list_buckets(
    db_conn: &DatabaseConnection,
    resource_group_id: Option<Uuid>,
    organization_id: Option<Uuid>,
) -> Result<Vec<BucketResponse>> {
    let rows = db::list(db_conn, resource_group_id, organization_id).await?;
    Ok(rows.into_iter().map(db::into_response).collect())
}

pub async fn update_bucket(
    db_conn: &DatabaseConnection,
    garage: &GarageClient,
    id: Uuid,
    req: UpdateBucketRequest,
) -> Result<Option<BucketResponse>> {
    let Some(existing) = db::get_by_id(db_conn, id).await? else {
        return Ok(None);
    };

    if req.quota_max_size.is_some() || req.quota_max_objects.is_some() {
        let Some(garage_bucket_id) = &existing.garage_bucket_id else {
            bail!("bucket has no garage_bucket_id, cannot update quota");
        };
        garage
            .update_bucket_quotas(garage_bucket_id, req.quota_max_size, req.quota_max_objects)
            .await?;
    }

    let updated = db::update(db_conn, id, &req).await?.map(db::into_response);
    Ok(updated)
}

pub async fn delete_bucket(
    db_conn: &DatabaseConnection,
    garage: &GarageClient,
    id: Uuid,
) -> Result<bool> {
    let Some(existing) = db::get_by_id(db_conn, id).await? else {
        return Ok(false);
    };

    if let Some(garage_bucket_id) = &existing.garage_bucket_id {
        garage.delete_bucket(garage_bucket_id).await?;
    }

    db::delete(db_conn, id).await?;
    log_info!("services::bucket", &format!("bucket deleted id={}", id));
    Ok(true)
}
