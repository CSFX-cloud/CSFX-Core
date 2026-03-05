use etcd_client::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementRecord {
    pub workload_id: Uuid,
    pub agent_id: Uuid,
    pub image: String,
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    pub disk_bytes: i64,
    pub scheduled_at: String,
}

pub async fn put_placement(
    etcd: &Arc<Mutex<Client>>,
    record: &PlacementRecord,
) -> Result<(), String> {
    let key = format!("/csf/placements/{}", record.workload_id);
    let value = serde_json::to_string(record)
        .map_err(|e| format!("Failed to serialize placement: {}", e))?;

    etcd.lock()
        .await
        .put(key, value, None)
        .await
        .map_err(|e| format!("Failed to write placement to etcd: {}", e))?;

    Ok(())
}

pub async fn delete_placement(
    etcd: &Arc<Mutex<Client>>,
    workload_id: Uuid,
) -> Result<(), String> {
    let key = format!("/csf/placements/{}", workload_id);

    etcd.lock()
        .await
        .delete(key, None)
        .await
        .map_err(|e| format!("Failed to delete placement from etcd: {}", e))?;

    Ok(())
}
