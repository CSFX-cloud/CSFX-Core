use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::server::AppState;

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
