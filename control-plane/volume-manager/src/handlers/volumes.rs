use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::models::volume::{
    AttachVolumeRequest, CreateSnapshotRequest, CreateVolumeRequest, ResizeVolumeRequest,
};
use crate::server::AppState;
use crate::services::volume::ServiceError;

pub async fn create_volume(
    State(state): State<AppState>,
    Json(req): Json<CreateVolumeRequest>,
) -> impl IntoResponse {
    match state.volume_service.create_volume(req).await {
        Ok(vol) => (StatusCode::CREATED, Json(serde_json::json!(vol))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn list_volumes(State(state): State<AppState>) -> impl IntoResponse {
    match state.volume_service.list_volumes().await {
        Ok(vols) => (StatusCode::OK, Json(serde_json::json!(vols))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn get_volume(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    match state.volume_service.get_volume(id).await {
        Ok(Some(vol)) => (StatusCode::OK, Json(serde_json::json!(vol))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "volume not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn delete_volume(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.volume_service.delete_volume(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => service_error_response(e),
    }
}

pub async fn attach_volume(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AttachVolumeRequest>,
) -> impl IntoResponse {
    match state.volume_service.attach_volume(id, req).await {
        Ok(vol) => (StatusCode::OK, Json(serde_json::json!(vol))).into_response(),
        Err(e) => service_error_response(e),
    }
}

pub async fn resize_volume(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ResizeVolumeRequest>,
) -> impl IntoResponse {
    match state.volume_service.resize_volume(id, req).await {
        Ok(vol) => (StatusCode::OK, Json(serde_json::json!(vol))).into_response(),
        Err(e) => service_error_response(e),
    }
}

pub async fn detach_volume(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.volume_service.detach_volume(id).await {
        Ok(vol) => (StatusCode::OK, Json(serde_json::json!(vol))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn create_snapshot(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateSnapshotRequest>,
) -> impl IntoResponse {
    match state.volume_service.create_snapshot(id, req).await {
        Ok(snap) => (StatusCode::CREATED, Json(serde_json::json!(snap))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn list_snapshots(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.volume_service.list_snapshots(id).await {
        Ok(snaps) => (StatusCode::OK, Json(serde_json::json!(snaps))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn delete_snapshot(
    State(state): State<AppState>,
    Path((id, snapshot_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    match state.volume_service.delete_snapshot(id, snapshot_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

fn service_error_response(error: ServiceError) -> axum::response::Response {
    let (status, message) = match error {
        ServiceError::Conflict(message) => (StatusCode::CONFLICT, message),
        ServiceError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
    };
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}
