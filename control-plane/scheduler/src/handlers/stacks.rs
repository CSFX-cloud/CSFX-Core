use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::{
    models::compose::{CreateStackRequest, RedeployStackRequest, StackResponse},
    server::AppState,
};

pub async fn create_stack(
    State(state): State<AppState>,
    Json(req): Json<CreateStackRequest>,
) -> impl IntoResponse {
    match state.scheduler.schedule_stack(req).await {
        Ok(resp) => (StatusCode::CREATED, Json(serde_json::json!(resp))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn get_stack(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    match crate::db::workload_stacks::get_by_id(&state.db, id).await {
        Ok(Some(model)) => (
            StatusCode::OK,
            Json(serde_json::json!(StackResponse {
                compose_source: crate::db::workload_stacks::display_source(&model),
                id: model.id,
                resource_group_id: model.resource_group_id,
                name: model.name,
                status: model.status,
                created_at: model.created_at.and_utc(),
                updated_at: model.updated_at.map(|dt| dt.and_utc()),
            })),
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_stack(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.scheduler.delete_stack(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn stop_stack(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    match state.scheduler.stop_stack(id).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

fn stack_error_status(message: &str) -> StatusCode {
    if message == crate::services::scheduler::STACK_BUSY {
        StatusCode::CONFLICT
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub async fn restart_stack(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.scheduler.restart_stack(id).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            stack_error_status(&e),
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn redeploy_stack(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<RedeployStackRequest>,
) -> impl IntoResponse {
    match state.scheduler.redeploy_stack(id, &req.compose_yaml).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            stack_error_status(&e),
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
