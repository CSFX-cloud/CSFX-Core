use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde_json::json;

use crate::AppState;

/// Pre-register agent (Admin only) - NEW WORKFLOW
pub async fn pre_register_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = if body.is_empty() {
        None
    } else {
        serde_json::from_str(&body).ok()
    };

    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::POST,
            "/admin/agents/pre-register",
            body_json,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// List pending (pre-registered) agents (Admin only)
pub async fn list_pending_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::GET,
            "/admin/agents/pending",
            None,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// Delete pending agent (Admin only)
pub async fn delete_pending_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::POST,
            &format!("/admin/agents/pending/{}", agent_id),
            None,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// Create token (Admin only) - DEPRECATED
pub async fn create_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = if body.is_empty() {
        None
    } else {
        serde_json::from_str(&body).ok()
    };

    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::POST,
            "/admin/tokens",
            body_json,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// List tokens (Admin only)
pub async fn list_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::GET,
            "/admin/tokens",
            None,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// Register agent
pub async fn register_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = if body.is_empty() {
        None
    } else {
        serde_json::from_str(&body).ok()
    };

    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::POST,
            "/agents/register",
            body_json,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// Agent heartbeat
pub async fn agent_heartbeat(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = if body.is_empty() {
        None
    } else {
        serde_json::from_str(&body).ok()
    };

    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::POST,
            &format!("/agents/{}/heartbeat", agent_id),
            body_json,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// List all agents (Admin)
pub async fn list_agents_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::GET,
            "/admin/agents",
            None,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// Get registry statistics
pub async fn get_statistics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    match state
        .service_client
        .forward_to_registry(
            reqwest::Method::GET,
            "/admin/statistics",
            None,
            Some(header_map),
        )
        .await
    {
        Ok((status, Some(response_body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(response_body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to registry: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Registry service unavailable",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// Registry health check
pub async fn registry_health(State(state): State<AppState>) -> impl IntoResponse {
    if state.service_client.check_registry_health().await {
        (StatusCode::OK, Json(json!({"status": "healthy"})))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "unavailable"})),
        )
    }
}

pub fn registry_routes() -> Router<AppState> {
    Router::new()
        // Admin routes - Pre-Registration (NEW WORKFLOW)
        .route("/registry/admin/agents/pre-register", post(pre_register_agent))
        .route("/registry/admin/agents/pending", get(list_pending_agents))
        .route("/registry/admin/agents/pending/:id", post(delete_pending_agent))
        // Admin routes - Token Management (DEPRECATED)
        .route("/registry/admin/tokens", post(create_token))
        .route("/registry/admin/tokens", get(list_tokens))
        // Admin routes - Agent Management
        .route("/registry/admin/agents", get(list_agents_admin))
        .route("/registry/admin/statistics", get(get_statistics))
        // Agent routes
        .route("/registry/agents/register", post(register_agent))
        .route("/registry/agents/:id/heartbeat", post(agent_heartbeat))
        // Health check
        .route("/registry/health", get(registry_health))
}
