use entity::entities::workloads;
use entity::{Buckets, Volumes, Workloads};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::{bucket, database, etcd, volume, workload};
use crate::{
    export::{engine::Exporter, ExportError},
    AppState,
};

const VM_RUNTIME_CLASS: &str = "vm";

pub async fn export(state: &AppState, exporter: &mut Exporter) -> Result<(), ExportError> {
    let db = &state.db_conn;
    database::export(exporter).await?;
    etcd::export(exporter).await?;
    for volume_row in Volumes::find().all(db).await? {
        volume::export(state, exporter, volume_row.id).await?;
    }
    let vms = Workloads::find()
        .filter(workloads::Column::RuntimeClass.eq(VM_RUNTIME_CLASS))
        .all(db)
        .await?;
    for vm in vms {
        workload::export(state, exporter, vm.id).await?;
    }
    for bucket_row in Buckets::find().all(db).await? {
        bucket::export(state, exporter, bucket_row.id).await?;
    }
    Ok(())
}
