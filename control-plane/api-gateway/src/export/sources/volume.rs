use entity::Volumes;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
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

fn row_name(volume_id: Uuid) -> String {
    format!("volumes/{volume_id}.json")
}

fn image_name(volume_id: Uuid) -> String {
    format!("volumes/{volume_id}.img")
}

pub async fn export(
    state: &AppState,
    exporter: &mut Exporter,
    volume_id: Uuid,
) -> Result<(), ExportError> {
    let volume = Volumes::find_by_id(volume_id)
        .one(&state.db_conn)
        .await?
        .ok_or_else(|| ExportError::ResourceNotFound(format!("volume {volume_id}")))?;
    exporter
        .add_bytes(
            row_name(volume_id),
            "row",
            Some(volume_id),
            serde_json::to_vec(&volume)?,
        )
        .await?;
    let response = state
        .service_client
        .stream_volume_manager(Method::GET, &format!("/volumes/{volume_id}/export"), None)
        .await
        .map_err(upstream)?;
    exporter
        .add_stream(
            image_name(volume_id),
            "volume",
            Some(volume_id),
            response_stream(response),
        )
        .await
}

fn import_path(row: &Value) -> Result<String, ExportError> {
    let name = row["name"].as_str().ok_or(ExportError::MalformedArtifact)?;
    let size_gb = row["size_gb"]
        .as_i64()
        .ok_or(ExportError::MalformedArtifact)?;
    let restored = format!("{name}{RESTORED_SUFFIX}");
    let mut path = format!(
        "/volumes/import?name={}&size_gb={size_gb}",
        utf8_percent_encode(&restored, NON_ALPHANUMERIC)
    );
    if let Some(group) = row["resource_group_id"].as_str() {
        path.push_str(&format!("&resource_group_id={group}"));
    }
    Ok(path)
}

pub async fn restore(
    state: &AppState,
    reader: &Reader,
    volume_id: Uuid,
) -> Result<Value, ExportError> {
    let row: Value = serde_json::from_slice(&reader.bytes(&row_name(volume_id)).await?)?;
    let image = reader.stream(&image_name(volume_id)).await?;
    let response = state
        .service_client
        .stream_volume_manager(Method::POST, &import_path(&row)?, Some(request_body(image)))
        .await
        .map_err(upstream)?;
    let created: Value = response.json().await.map_err(upstream)?;
    Ok(json!({ "volume_id": created["id"] }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_path_encodes_name_and_keeps_group() {
        let row = json!({
            "name": "data disk/1",
            "size_gb": 20,
            "resource_group_id": "6f1f2a52-0000-4000-8000-000000000001"
        });
        assert_eq!(
            import_path(&row).unwrap(),
            "/volumes/import?name=data%20disk%2F1%2Drestored&size_gb=20&resource_group_id=6f1f2a52-0000-4000-8000-000000000001"
        );
    }

    #[test]
    fn import_path_rejects_incomplete_rows() {
        assert!(import_path(&json!({ "name": "x" })).is_err());
    }
}
