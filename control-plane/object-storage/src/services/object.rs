use anyhow::{bail, Result};
use axum::body::Bytes;
use futures_util::{Stream, StreamExt};
use sea_orm::DatabaseConnection;
use std::{fmt::Display, time::Duration};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    archive::{self, EntryReader},
    crypto::SecretBox,
    db::buckets as buckets_db,
    garage::S3Client,
    models::{ListObjectsResponse, ObjectEntry, PresignResponse},
};

const PRESIGN_EXPIRY_SECONDS: u64 = 900;
const EXPORT_CHANNEL_DEPTH: usize = 8;
const MAX_IMPORT_ENTRY_BYTES: u64 = 256 << 20;

pub type ExportChunk = Result<Bytes, std::io::Error>;

struct MasterCredentials {
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
}

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

pub async fn purge_objects(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
) -> Result<()> {
    let (global_alias, access_key_id, secret_access_key) =
        resolve_master_credentials(db, secret_box, bucket_id).await?;
    let mut continuation_token: Option<String> = None;
    loop {
        let page = s3
            .list_objects(
                &global_alias,
                &access_key_id,
                &secret_access_key,
                "",
                "",
                continuation_token.as_deref(),
            )
            .await?;
        for object in &page.objects {
            s3.delete_object(
                &global_alias,
                &access_key_id,
                &secret_access_key,
                &object.key,
            )
            .await?;
        }
        continuation_token = page.next_continuation_token;
        if continuation_token.is_none() {
            return Ok(());
        }
    }
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
    public_origin: &str,
) -> Result<Option<PresignResponse>> {
    presign(db, s3, secret_box, bucket_id, key, public_origin, "PUT").await
}

pub async fn presign_download(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    key: &str,
    public_origin: &str,
) -> Result<Option<PresignResponse>> {
    presign(db, s3, secret_box, bucket_id, key, public_origin, "GET").await
}

async fn presign(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    key: &str,
    public_origin: &str,
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
        public_origin.trim_end_matches('/'),
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

async fn resolve_credentials(
    db: &DatabaseConnection,
    secret_box: &SecretBox,
    bucket_id: Uuid,
) -> Result<Option<MasterCredentials>> {
    match resolve_master_credentials(db, secret_box, bucket_id).await {
        Ok((bucket, access_key_id, secret_access_key)) => Ok(Some(MasterCredentials {
            bucket,
            access_key_id,
            secret_access_key,
        })),
        Err(e) if e.to_string() == "bucket not found" => Ok(None),
        Err(e) => Err(e),
    }
}

async fn emit(tx: &mpsc::Sender<ExportChunk>, bytes: Bytes) -> Result<()> {
    tx.send(Ok(bytes))
        .await
        .map_err(|_| anyhow::anyhow!("export consumer disconnected"))
}

async fn send_object(
    s3: &S3Client,
    credentials: &MasterCredentials,
    tx: &mpsc::Sender<ExportChunk>,
    object: &crate::garage::s3_client::S3Object,
) -> Result<()> {
    let size = u64::try_from(object.size)?;
    emit(tx, Bytes::from(archive::entry_header(&object.key, size))).await?;
    let mut body = s3
        .get_object(
            &credentials.bucket,
            &credentials.access_key_id,
            &credentials.secret_access_key,
            &object.key,
        )
        .await?
        .bytes_stream();
    let mut sent = 0u64;
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        sent += chunk.len() as u64;
        emit(tx, chunk).await?;
    }
    if sent != size {
        bail!("object {} changed during export", object.key);
    }
    emit(tx, Bytes::from(vec![0u8; archive::padding(size)])).await
}

async fn produce(
    s3: &S3Client,
    credentials: &MasterCredentials,
    tx: &mpsc::Sender<ExportChunk>,
) -> Result<()> {
    let mut token: Option<String> = None;
    loop {
        let page = s3
            .list_objects(
                &credentials.bucket,
                &credentials.access_key_id,
                &credentials.secret_access_key,
                "",
                "",
                token.as_deref(),
            )
            .await?;
        for object in &page.objects {
            send_object(s3, credentials, tx, object).await?;
        }
        token = page.next_continuation_token;
        if token.is_none() {
            break;
        }
    }
    emit(tx, Bytes::from_static(&archive::TRAILER)).await
}

pub async fn export_bucket(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
) -> Result<Option<mpsc::Receiver<ExportChunk>>> {
    let Some(credentials) = resolve_credentials(db, secret_box, bucket_id).await? else {
        return Ok(None);
    };
    let (tx, rx) = mpsc::channel(EXPORT_CHANNEL_DEPTH);
    let s3 = s3.clone();
    tokio::spawn(async move {
        if let Err(error) = produce(&s3, &credentials, &tx).await {
            tracing::error!(bucket_id = %bucket_id, error = %error, "bucket export failed");
            let _ = tx.send(Err(std::io::Error::other(error.to_string()))).await;
        }
    });
    Ok(Some(rx))
}

pub async fn import_bucket<S, E>(
    db: &DatabaseConnection,
    s3: &S3Client,
    secret_box: &SecretBox,
    bucket_id: Uuid,
    archive_stream: S,
) -> Result<Option<u64>>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    let Some(credentials) = resolve_credentials(db, secret_box, bucket_id).await? else {
        return Ok(None);
    };
    let mut reader = EntryReader::new(archive_stream, MAX_IMPORT_ENTRY_BYTES);
    let mut imported = 0u64;
    while let Some(entry) = reader.next_entry().await? {
        s3.put_object(
            &credentials.bucket,
            &credentials.access_key_id,
            &credentials.secret_access_key,
            &entry.key,
            entry.data,
        )
        .await?;
        imported += 1;
    }
    Ok(Some(imported))
}
