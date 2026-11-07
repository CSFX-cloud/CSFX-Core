use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::{
    db,
    models::workload::{CreateWorkloadRequest, DesiredState, UpdateWorkloadRequest},
    server::AppState,
};

pub async fn create_workload(
    State(state): State<AppState>,
    Json(req): Json<CreateWorkloadRequest>,
) -> impl IntoResponse {
    if let Err(e) = req.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response();
    }
    match state.scheduler.schedule_replicas(req).await {
        Ok(mut responses) if responses.len() == 1 => (
            StatusCode::CREATED,
            Json(serde_json::json!(responses.remove(0))),
        )
            .into_response(),
        Ok(responses) => (StatusCode::CREATED, Json(serde_json::json!(responses))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn list_workloads(State(state): State<AppState>) -> impl IntoResponse {
    match state.scheduler.list_workloads().await {
        Ok(workloads) => (StatusCode::OK, Json(serde_json::json!(workloads))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn delete_workload(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.scheduler.delete_workload(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn stop_workload(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match db::workloads::set_desired_state(&state.db, id, DesiredState::Stopped).await {
        Ok(model) => {
            if let Some(agent_id) = model.assigned_agent_id {
                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
            }
            (StatusCode::OK, Json(serde_json::json!(model))).into_response()
        }
        Err(sea_orm::DbErr::RecordNotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn claim_requested_volumes(
    state: &AppState,
    workload_id: Uuid,
    req: &UpdateWorkloadRequest,
) -> Result<(), axum::response::Response> {
    let Some(mounts) = &req.volume_mounts else {
        return Ok(());
    };
    let workload = match db::workloads::get_by_id(&state.db, workload_id).await {
        Ok(Some(workload)) => workload,
        Ok(None) => return Err(StatusCode::NOT_FOUND.into_response()),
        Err(e) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ))
        }
    };
    let Some(agent_id) = workload.assigned_agent_id else {
        return Ok(());
    };
    let volume_ids: Vec<Uuid> = mounts.iter().map(|mount| mount.volume_id).collect();
    match db::volumes::claim(&state.db, &volume_ids, agent_id, workload_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(error_response(
            StatusCode::CONFLICT,
            "A volume is attached to another workload or node".to_string(),
        )),
        Err(e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )),
    }
}

fn error_response(status: StatusCode, message: String) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

pub async fn update_workload(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWorkloadRequest>,
) -> impl IntoResponse {
    if let Err(response) = claim_requested_volumes(&state, id, &req).await {
        return response;
    }
    match db::workloads::update_spec(&state.db, id, &req).await {
        Ok(model) => {
            if let Some(agent_id) = model.assigned_agent_id {
                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
            }
            (StatusCode::OK, Json(serde_json::json!(model))).into_response()
        }
        Err(sea_orm::DbErr::RecordNotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn restart_workload(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match db::workloads::request_restart(&state.db, id).await {
        Ok(model) => {
            if let Some(agent_id) = model.assigned_agent_id {
                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
            }
            (StatusCode::OK, Json(serde_json::json!(model))).into_response()
        }
        Err(sea_orm::DbErr::RecordNotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
