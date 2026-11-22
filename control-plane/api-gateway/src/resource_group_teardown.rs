use anyhow::{bail, Context, Result};
use entity::{buckets, volumes, workloads, Buckets, Volumes, Workloads};
use reqwest::{Method, StatusCode};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

use crate::{service_client::Service, vault_service, AppState};

async fn delete_through(
    state: &AppState,
    service: Service,
    path: String,
    resource_group_id: Uuid,
) -> Result<()> {
    let (status, _) = state
        .service_client
        .call_json(service, Method::DELETE, &path, None)
        .await
        .with_context(|| format!("delete {path} failed"))?;
    if status.is_success() || status == StatusCode::NOT_FOUND {
        tracing::info!(resource_group_id = %resource_group_id, path = %path, "contained resource deleted");
        return Ok(());
    }
    bail!("delete {path} failed status={status}")
}

async fn delete_all(
    state: &AppState,
    service: Service,
    resource_group_id: Uuid,
    ids: Vec<Uuid>,
    collection: &str,
) -> Result<()> {
    for id in ids {
        delete_through(
            state,
            service,
            format!("/{collection}/{id}"),
            resource_group_id,
        )
        .await?;
    }
    Ok(())
}

pub async fn teardown(state: &AppState, resource_group_id: Uuid) -> Result<()> {
    let workload_ids = Workloads::find()
        .select_only()
        .column(workloads::Column::Id)
        .filter(workloads::Column::ResourceGroupId.eq(resource_group_id))
        .into_tuple::<Uuid>()
        .all(&state.db_conn)
        .await?;
    delete_all(
        state,
        Service::Scheduler,
        resource_group_id,
        workload_ids,
        "workloads",
    )
    .await?;

    let volume_ids = Volumes::find()
        .select_only()
        .column(volumes::Column::Id)
        .filter(volumes::Column::ResourceGroupId.eq(resource_group_id))
        .into_tuple::<Uuid>()
        .all(&state.db_conn)
        .await?;
    delete_all(
        state,
        Service::VolumeManager,
        resource_group_id,
        volume_ids,
        "volumes",
    )
    .await?;

    let bucket_ids = Buckets::find()
        .select_only()
        .column(buckets::Column::Id)
        .filter(buckets::Column::ResourceGroupId.eq(resource_group_id))
        .into_tuple::<Uuid>()
        .all(&state.db_conn)
        .await?;
    delete_all(
        state,
        Service::ObjectStorage,
        resource_group_id,
        bucket_ids,
        "buckets",
    )
    .await?;

    vault_service::teardown_resource_group(state, resource_group_id).await?;
    Ok(())
}
