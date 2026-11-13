use anyhow::{bail, Result};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use uuid::Uuid;

use crate::{
    crypto::SecretBox,
    db::buckets as buckets_db,
    garage::S3Client,
    models::{ListObjectsResponse, ObjectEntry, PresignResponse},
};

const PRESIGN_EXPIRY_SECONDS: u64 = 900;

async fn resolve_master_credentials(
    db: &DatabaseConnection,
    secret_box: &SecretBox,
    bucket_id: Uuid,
) -> Result<(String, String, String)> {
    let Some(bucket) = buckets_db::get_by_id(db, bucket_id).await? else {
        bail!("bucket not found");
    };

    let (Some(access_key_id), Some(encrypted_secret)) =
        (bucket.master_key_id, bucket.master_key_secret_encrypted)
    else {
        bail!("bucket has no master key configured");
    };

    let secret_access_key = secret_box.decrypt(&encrypted_secret)?;

    Ok((bucket.global_alias, access_key_id, secret_access_key))
}

pub async fn list_objects(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    prefix: &str,
    continuation_token: Option<&str>,
) -> Result<Option<ListObjectsResponse>> {
    let (global_alias, access_key_id, secret_access_key) =
        match resolve_master_credentials(db, secret_box, bucket_id).await {
            Ok(creds) => creds,
            Err(e) if e.to_string() == "bucket not found" => return Ok(None),
            Err(e) => return Err(e),
        };

    let result = s3
        .list_objects(
            &global_alias,
            &access_key_id,
            &secret_access_key,
            prefix,
            "/",
            continuation_token,
        )
        .await?;

    Ok(Some(ListObjectsResponse {
        objects: result
            .objects
            .into_iter()
            .map(|o| ObjectEntry {
                key: o.key,
                size: o.size,
                last_modified: o.last_modified,
            })
            .collect(),
        folders: result.common_prefixes,
        next_continuation_token: result.next_continuation_token,
    }))
}

pub async fn delete_object(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    key: &str,
) -> Result<bool> {
    let (global_alias, access_key_id, secret_access_key) =
        match resolve_master_credentials(db, secret_box, bucket_id).await {
            Ok(creds) => creds,
            Err(e) if e.to_string() == "bucket not found" => return Ok(false),
            Err(e) => return Err(e),
        };

    s3.delete_object(&global_alias, &access_key_id, &secret_access_key, key)
        .await?;

    Ok(true)
}

pub async fn presign_upload(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    key: &str,
) -> Result<Option<PresignResponse>> {
    presign(db, s3, secret_box, bucket_id, key, "PUT").await
}

pub async fn presign_download(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    key: &str,
) -> Result<Option<PresignResponse>> {
    presign(db, s3, secret_box, bucket_id, key, "GET").await
}

async fn presign(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    key: &str,
    method: &str,
) -> Result<Option<PresignResponse>> {
    let (global_alias, access_key_id, secret_access_key) =
        match resolve_master_credentials(db, secret_box, bucket_id).await {
            Ok(creds) => creds,
            Err(e) if e.to_string() == "bucket not found" => return Ok(None),
            Err(e) => return Err(e),
        };

    let expires_in = Duration::from_secs(PRESIGN_EXPIRY_SECONDS);
    let url = s3.presign_url(
        method,
        &global_alias,
        key,
        &access_key_id,
        &secret_access_key,
        expires_in,
    )?;

    Ok(Some(PresignResponse {
        url,
        expires_in_seconds: PRESIGN_EXPIRY_SECONDS,
    }))
}
