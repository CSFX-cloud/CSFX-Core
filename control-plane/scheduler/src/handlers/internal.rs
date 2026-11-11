use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::models::workload::VolumeUsage;
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct RescheduleRequest {
    pub workload_ids: Vec<Uuid>,
}

pub async fn reschedule_agent_workloads(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<RescheduleRequest>,
) -> impl IntoResponse {
    match state
        .scheduler
        .reschedule_from_agent(agent_id, &req.workload_ids)
        .await
    {
        Ok(results) => (StatusCode::OK, Json(serde_json::json!(results))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ContainerStatusUpdate {
    pub workload_id: Uuid,
    pub container_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchStatusRequest {
    pub statuses: Vec<ContainerStatusUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct WorkloadStatsUpdate {
    pub workload_id: Uuid,
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_bytes: Option<i64>,
    pub network_rx_bytes: Option<i64>,
    pub network_tx_bytes: Option<i64>,
    #[serde(default)]
    pub volume_usage: Option<Vec<VolumeUsage>>,
}

#[derive(Debug, Deserialize)]
pub struct BatchStatsRequest {
    pub stats: Vec<WorkloadStatsUpdate>,
}

pub async fn update_workload_stats(
    State(state): State<AppState>,
    Json(req): Json<BatchStatsRequest>,
) -> impl IntoResponse {
    for update in &req.stats {
        if let Err(e) = crate::db::workloads::update_stats(
            &state.db,
            update.workload_id,
            update.cpu_usage_percent,
            update.memory_usage_bytes,
            update.network_rx_bytes,
            update.network_tx_bytes,
            update.volume_usage.clone(),
        )
        .await
        {
            crate::log_warn!(
                "internal",
                &format!(
                    "Failed to update workload stats workload_id={} err={}",
                    update.workload_id, e
                )
            );
        }
    }

    StatusCode::NO_CONTENT
}

pub async fn ack_workload_restart(
    State(state): State<AppState>,
    Path(workload_id): Path<Uuid>,
) -> impl IntoResponse {
    match crate::db::workloads::clear_restart_request(&state.db, workload_id).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(sea_orm::DbErr::RecordNotFound(_)) => StatusCode::NOT_FOUND,
        Err(e) => {
            crate::log_warn!(
                "internal",
                &format!(
                    "Failed to ack workload restart workload_id={} err={}",
                    workload_id, e
                )
            );
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn update_container_statuses(
    State(state): State<AppState>,
    Json(req): Json<BatchStatusRequest>,
) -> impl IntoResponse {
    for update in &req.statuses {
        if let Err(e) = crate::db::workloads::update_container_status(
            &state.db,
            update.workload_id,
            &update.container_id,
            &update.status,
        )
        .await
        {
            crate::log_warn!(
                "internal",
                &format!(
                    "Failed to update container status workload_id={} err={}",
                    update.workload_id, e
                )
            );
        } else {
            crate::log_info!(
                "internal",
                &format!(
                    "Container status updated workload_id={} status={}",
                    update.workload_id, update.status
                )
            );
        }
    }

    StatusCode::NO_CONTENT
}
