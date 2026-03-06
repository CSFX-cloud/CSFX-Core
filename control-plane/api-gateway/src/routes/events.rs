use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;

use crate::{auth::middleware::AuthenticatedUser, AppState};

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
                Json(json!({ "error": "Failover controller unavailable", "details": e.to_string() })),
            ))
        }
    }
}

pub async fn list_events(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    proxy_to_failover(&state, reqwest::Method::GET, "/events", None, Some(header_map)).await
}

pub fn events_routes() -> Router<AppState> {
    Router::new().route("/events", get(list_events))
}
