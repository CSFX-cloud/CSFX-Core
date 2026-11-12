use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

use crate::models::workload::{VolumeUsage, WorkloadStatus};
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
    pub agent_id: Uuid,
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
    pub agent_id: Uuid,
    pub stats: Vec<WorkloadStatsUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct RestartAckRequest {
    pub agent_id: Uuid,
}

async fn owned_workloads(
    state: &AppState,
    agent_id: Uuid,
    workload_ids: Vec<Uuid>,
) -> Result<HashSet<Uuid>, sea_orm::DbErr> {
    crate::db::workloads::assigned_to_agent(&state.db, agent_id, &workload_ids).await
}

fn log_foreign_workload(agent_id: Uuid, workload_id: Uuid, operation: &str) {
    crate::log_warn!(
        "internal",
        &format!(
            "Rejected update for workload not assigned to agent agent_id={} workload_id={} operation={}",
            agent_id, workload_id, operation
        )
    );
}

pub async fn update_workload_stats(
    State(state): State<AppState>,
    Json(req): Json<BatchStatsRequest>,
) -> impl IntoResponse {
    let ids = req.stats.iter().map(|update| update.workload_id).collect();
    let owned = match owned_workloads(&state, req.agent_id, ids).await {
        Ok(owned) => owned,
        Err(e) => return database_failure("stats", &e),
    };
    for update in &req.stats {
        if !owned.contains(&update.workload_id) {
            log_foreign_workload(req.agent_id, update.workload_id, "stats");
            continue;
        }
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
    Json(req): Json<RestartAckRequest>,
) -> impl IntoResponse {
    match crate::db::workloads::get_by_id(&state.db, workload_id).await {
        Ok(Some(workload)) if workload.assigned_agent_id != Some(req.agent_id) => {
            log_foreign_workload(req.agent_id, workload_id, "restart_ack");
            return StatusCode::FORBIDDEN;
        }
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::NOT_FOUND,
        Err(e) => return database_failure("restart_ack", &e),
    }
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
    let ids = req
        .statuses
        .iter()
        .map(|update| update.workload_id)
        .collect();
    let owned = match owned_workloads(&state, req.agent_id, ids).await {
        Ok(owned) => owned,
        Err(e) => return database_failure("status", &e),
    };
    for update in &req.statuses {
        if !owned.contains(&update.workload_id) {
            log_foreign_workload(req.agent_id, update.workload_id, "status");
            continue;
        }
        let Some(status) = WorkloadStatus::parse(&update.status) else {
            crate::log_warn!(
                "internal",
                &format!(
                    "Rejected unknown workload status agent_id={} workload_id={}",
                    req.agent_id, update.workload_id
                )
            );
            continue;
        };
        if let Err(e) = crate::db::workloads::update_container_status(
            &state.db,
            update.workload_id,
            &update.container_id,
            status.as_str(),
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

fn database_failure(operation: &str, error: &sea_orm::DbErr) -> StatusCode {
    crate::log_warn!(
        "internal",
        &format!(
            "Failed to check workload ownership operation={} err={}",
            operation, error
        )
    );
    StatusCode::INTERNAL_SERVER_ERROR
}
