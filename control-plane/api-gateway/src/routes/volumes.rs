use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use serde_json::json;

use crate::{auth::rbac::{CanManageVolumes, CanViewVolumes}, AppState};

async fn proxy_to_volume_manager(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    headers: Option<Vec<(String, String)>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state
        .service_client
        .forward_to_volume_manager(method, path, body, headers)
        .await
    {
        Ok((status, Some(body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to volume-manager: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "Volume Manager service unavailable", "details": e.to_string() })),
            ))
        }
    }
}

fn header_vec(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect()
}

pub async fn create_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::POST, "/volumes", body_json, Some(header_map)).await
}

pub async fn list_volumes(
    CanViewVolumes(_claims): CanViewVolumes,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::GET, "/volumes", None, Some(header_map)).await
}

pub async fn get_volume(
    CanViewVolumes(_claims): CanViewVolumes,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::GET, &format!("/volumes/{}", id), None, Some(header_map)).await
}

pub async fn delete_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::DELETE, &format!("/volumes/{}", id), None, Some(header_map)).await
}

pub async fn attach_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::POST, &format!("/volumes/{}/attach", id), body_json, Some(header_map)).await
}

pub async fn detach_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::POST, &format!("/volumes/{}/detach", id), None, Some(header_map)).await
}

pub async fn create_snapshot(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::POST, &format!("/volumes/{}/snapshots", id), body_json, Some(header_map)).await
}

pub async fn list_snapshots(
    CanViewVolumes(_claims): CanViewVolumes,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_volume_manager(&state, reqwest::Method::GET, &format!("/volumes/{}/snapshots", id), None, Some(header_map)).await
}

pub fn volumes_routes() -> Router<AppState> {
    Router::new()
        .route("/volumes", post(create_volume))
        .route("/volumes", get(list_volumes))
        .route("/volumes/:id", get(get_volume))
        .route("/volumes/:id", delete(delete_volume))
        .route("/volumes/:id/attach", post(attach_volume))
        .route("/volumes/:id/detach", post(detach_volume))
        .route("/volumes/:id/snapshots", post(create_snapshot))
        .route("/volumes/:id/snapshots", get(list_snapshots))
}
