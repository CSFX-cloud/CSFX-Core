use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{delete, get},
    Router,
};
use serde_json::json;

use crate::{auth::rbac::{CanManageNetworks, CanViewNetworks}, AppState};

async fn proxy_to_sdn(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    headers: Option<Vec<(String, String)>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state
        .service_client
        .forward_to_sdn_controller(method, path, body, headers)
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
            tracing::error!("Failed to forward request to sdn-controller: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "SDN controller unavailable", "details": e.to_string() })),
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

pub async fn list_networks(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::GET, "/networks", None, Some(header_map)).await
}

pub async fn create_network(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::POST, "/networks", Some(body), Some(header_map)).await
}

pub async fn get_network(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::GET, &format!("/networks/{}", id), None, Some(header_map)).await
}

pub async fn delete_network(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::DELETE, &format!("/networks/{}", id), None, Some(header_map)).await
}

pub async fn list_policies(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::GET, &format!("/networks/{}/policies", id), None, Some(header_map)).await
}

pub async fn create_policy(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::POST, &format!("/networks/{}/policies", id), Some(body), Some(header_map)).await
}

pub async fn list_members(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::GET, &format!("/networks/{}/members", id), None, Some(header_map)).await
}

pub async fn add_member(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::POST, &format!("/networks/{}/members", id), Some(body), Some(header_map)).await
}

pub async fn remove_member(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path((id, workload_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_sdn(&state, reqwest::Method::DELETE, &format!("/networks/{}/members/{}", id, workload_id), None, Some(header_map)).await
}

pub fn networks_routes() -> Router<AppState> {
    Router::new()
        .route("/networks", get(list_networks).post(create_network))
        .route("/networks/:id", get(get_network).delete(delete_network))
        .route("/networks/:id/policies", get(list_policies).post(create_policy))
        .route("/networks/:id/members", get(list_members).post(add_member))
        .route("/networks/:id/members/:workload_id", delete(remove_member))
}
