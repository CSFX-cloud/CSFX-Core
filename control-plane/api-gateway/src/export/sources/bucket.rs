use entity::Buckets;
use reqwest::Method;
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use uuid::Uuid;

use super::{request_body, response_stream, upstream};
use crate::{
    export::{
        engine::{Exporter, Reader},
        ExportError,
    },
    AppState,
};

const RESTORED_SUFFIX: &str = "-restored";
const SECRET_FIELDS: [&str; 2] = ["master_key_id", "master_key_secret_encrypted"];

fn row_name(bucket_id: Uuid) -> String {
    format!("buckets/{bucket_id}.json")
}

fn archive_name(bucket_id: Uuid) -> String {
    format!("buckets/{bucket_id}.tar")
}

fn row_without_secrets(row: &impl serde::Serialize) -> Result<Vec<u8>, ExportError> {
    let mut value = serde_json::to_value(row)?;
    if let Some(object) = value.as_object_mut() {
        for field in SECRET_FIELDS {
            object.remove(field);
        }
    }
    Ok(serde_json::to_vec(&value)?)
}

pub async fn export(
    state: &AppState,
    exporter: &mut Exporter,
    bucket_id: Uuid,
) -> Result<(), ExportError> {
    let bucket = Buckets::find_by_id(bucket_id)
        .one(&state.db_conn)
        .await?
        .ok_or_else(|| ExportError::ResourceNotFound(format!("bucket {bucket_id}")))?;
    exporter
        .add_bytes(
            row_name(bucket_id),
            "row",
            Some(bucket_id),
            row_without_secrets(&bucket)?,
        )
        .await?;
    let response = state
        .service_client
        .stream_object_storage(Method::GET, &format!("/buckets/{bucket_id}/export"), None)
        .await
        .map_err(upstream)?;
    exporter
        .add_stream(
            archive_name(bucket_id),
            "bucket",
            Some(bucket_id),
            response_stream(response),
        )
        .await
}

fn create_request(row: &Value) -> Value {
    let name = row["name"].as_str().unwrap_or("bucket");
    json!({
        "name": format!("{name}{RESTORED_SUFFIX}"),
        "exposure": row["exposure"],
        "quota_max_size": row["quota_max_size"],
        "quota_max_objects": row["quota_max_objects"],
        "organization_id": row["organization_id"],
        "resource_group_id": row["resource_group_id"],
    })
}

async fn create(state: &AppState, row: &Value) -> Result<Uuid, ExportError> {
    let (status, body) = state
        .service_client
        .forward_to_object_storage(Method::POST, "/buckets", Some(create_request(row)), None)
        .await
        .map_err(upstream)?;
    let id = body
        .as_ref()
        .and_then(|body| body["id"].as_str())
        .and_then(|id| Uuid::parse_str(id).ok());
    match (status.is_success(), id) {
        (true, Some(id)) => Ok(id),
        _ => Err(upstream(format!("bucket creation answered {status}"))),
    }
}

async fn discard(state: &AppState, bucket_id: Uuid) {
    let result = state
        .service_client
        .forward_to_object_storage(Method::DELETE, &format!("/buckets/{bucket_id}"), None, None)
        .await;
    if let Err(error) = result {
        tracing::warn!(bucket_id = %bucket_id, error = %error, "failed to remove partially restored bucket");
    }
}

pub async fn restore(
    state: &AppState,
    reader: &Reader,
    bucket_id: Uuid,
) -> Result<Value, ExportError> {
    let row: Value = serde_json::from_slice(&reader.bytes(&row_name(bucket_id)).await?)?;
    let archive = reader.stream(&archive_name(bucket_id)).await?;
    let created = create(state, &row).await?;
    let path = format!("/buckets/{created}/import");
    let result = state
        .service_client
        .stream_object_storage(Method::POST, &path, Some(request_body(archive)))
        .await;
    let response = match result {
        Ok(response) => response,
        Err(error) => {
            discard(state, created).await;
            return Err(upstream(error));
        }
    };
    let imported: Value = response.json().await.map_err(upstream)?;
    Ok(json!({ "bucket_id": created, "objects": imported["objects"] }))
}
