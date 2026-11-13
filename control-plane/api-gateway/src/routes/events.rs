use axum::{
    body::Body,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageSystem, CanViewAgents},
    routes::paged_proxy::{proxy_list, with_query},
    service_client::ListTarget,
    AppState,
};

async fn proxy_to_failover(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    headers: Option<Vec<(String, String)>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state
        .service_client
        .forward_to_failover_controller(method, path, body, headers)
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
            tracing::error!("Failed to forward request to failover-controller: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(
                    json!({ "error": "Failover controller unavailable", "details": e.to_string() }),
                ),
            ))
        }
    }
}

pub async fn list_events(
    CanViewAgents(_claims): CanViewAgents,
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let path = with_query("/events", query);
    proxy_list(&state, ListTarget::FailoverController, &path, &headers).await
}

pub async fn resolve_event(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    proxy_to_failover(
        &state,
        reqwest::Method::POST,
        &format!("/events/{}/resolve", event_id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn set_maintenance(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    proxy_to_failover(
        &state,
        reqwest::Method::POST,
        &format!("/agents/{}/maintenance", agent_id),
        Some(body),
        Some(header_map),
    )
    .await
}

pub async fn clear_maintenance(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    proxy_to_failover(
        &state,
        reqwest::Method::DELETE,
        &format!("/agents/{}/maintenance", agent_id),
        None,
        Some(header_map),
    )
    .await
}

pub fn events_routes() -> Router<AppState> {
    Router::new()
        .route("/events", get(list_events))
        .route("/events/{event_id}/resolve", post(resolve_event))
        .route("/agents/{agent_id}/maintenance", post(set_maintenance))
        .route("/agents/{agent_id}/maintenance", delete(clear_maintenance))
}
