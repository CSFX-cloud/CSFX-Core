use entity::entities::{buckets, volumes, workloads};
use entity::{Buckets, Volumes, Workloads};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use uuid::Uuid;

use super::{bucket, volume, workload};
use crate::{
    export::{
        engine::{Exporter, Reader},
        ExportError,
    },
    AppState,
};

pub async fn export(
    state: &AppState,
    exporter: &mut Exporter,
    group_id: Uuid,
) -> Result<(), ExportError> {
    let db = &state.db_conn;
    let workload_ids = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(group_id))
        .all(db)
        .await?;
    for workload in workload_ids {
        workload::export(state, exporter, workload.id).await?;
    }
    let volume_rows = Volumes::find()
        .filter(volumes::Column::ResourceGroupId.eq(group_id))
        .all(db)
        .await?;
    for volume in volume_rows {
        volume::export(state, exporter, volume.id).await?;
    }
    let bucket_rows = Buckets::find()
        .filter(buckets::Column::ResourceGroupId.eq(group_id))
        .all(db)
        .await?;
    for bucket in bucket_rows {
        bucket::export(state, exporter, bucket.id).await?;
    }
    Ok(())
}

pub async fn restore(
    state: &AppState,
    reader: &Reader,
    requested_agent: Option<Uuid>,
) -> Result<Value, ExportError> {
    let mut restored = Vec::new();
    for artifact in &reader.manifest.artifacts {
        let Some(id) = artifact.source_id else {
            continue;
        };
        let outcome = match artifact.kind.as_str() {
            "volume" => volume::restore(state, reader, id).await,
            "bucket" => bucket::restore(state, reader, id).await,
            "disk" => workload::restore(state, reader, id, requested_agent).await,
            _ => continue,
        };
        let outcome = outcome.map_err(|error| {
            ExportError::PartialRestore(format!(
                "{} {id} failed after {} restored: {error}",
                artifact.kind,
                restored.len()
            ))
        })?;
        restored.push(json!({ "kind": artifact.kind, "source_id": id, "result": outcome }));
    }
    Ok(json!({ "restored": restored }))
}
