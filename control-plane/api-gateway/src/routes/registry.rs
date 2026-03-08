use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use crate::{auth::middleware::AuthenticatedUser, AppState};

// ==================== Request/Response Schemas for OpenAPI ====================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PreRegisterAgentRequest {
    /// Name of the agent to pre-register
    pub name: String,
    /// Hostname of the agent
    pub hostname: String,
    /// Expected OS type (e.g., "NixOS", "Ubuntu")
    pub expected_os_type: Option<String>,
    /// Expected architecture (e.g., "x86_64", "aarch64")
    pub expected_architecture: Option<String>,
    /// Optional metadata tags
    pub tags: Option<std::collections::HashMap<String, String>>,
    /// Token time-to-live in hours (default: 24)
    pub ttl_hours: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PreRegisterAgentResponse {
    /// Pre-registered agent ID
    pub agent_id: String,
    /// Registration token for agent to use
    pub registration_token: String,
    /// Token expiration timestamp
    pub token_expires_at: String,
    /// Success message
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterAgentRequest {
    /// Registration token received during pre-registration
    pub registration_token: String,
    /// Agent name (must match pre-registration)
    pub name: String,
    /// Agent hostname (must match pre-registration)
    pub hostname: String,
    /// Operating system type
    pub os_type: String,
    /// Operating system version
    pub os_version: String,
    /// System architecture
    pub architecture: String,
    /// Agent software version
    pub agent_version: String,
    /// Optional metadata tags
    pub tags: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterAgentResponse {
    /// Registered agent ID
    pub agent_id: String,
    /// API key for subsequent authentication
    pub api_key: String,
    /// Success message
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HeartbeatRequest {
    /// Optional agent status update
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AgentStatistics {
    /// Total number of agents
    pub total: usize,
    /// Number of online agents
    pub online: usize,
    /// Number of offline agents
    pub offline: usize,
    /// Number of degraded agents
    pub degraded: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
}

// ==================== API Endpoints ====================

/// Pre-register agent (Admin only) - NEW WORKFLOW
#[utoipa::path(
    post,
    path = "/registry/admin/agents/pre-register",
    request_body = PreRegisterAgentRequest,
    responses(
        (status = 200, description = "Agent pre-registered successfully", body = PreRegisterAgentResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn pre_register_agent(
    AuthenticatedUser(_claims): AuthenticatedUser,
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
#[utoipa::path(
    get,
    path = "/registry/admin/agents/pending",
    responses(
        (status = 200, description = "List of pending agents retrieved successfully"),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_pending_agents(
    AuthenticatedUser(_claims): AuthenticatedUser,
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
#[utoipa::path(
    post,
    path = "/registry/admin/agents/pending/{id}",
    params(
        ("id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 204, description = "Pending agent deleted successfully"),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_pending_agent(
    AuthenticatedUser(_claims): AuthenticatedUser,
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
#[deprecated(note = "Use pre_register_agent instead")]
#[utoipa::path(
    post,
    path = "/registry/admin/tokens",
    responses(
        (status = 400, description = "Endpoint deprecated - use pre_register_agent", body = ErrorResponse),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Admin (Deprecated)",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_token(
    AuthenticatedUser(_claims): AuthenticatedUser,
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
#[utoipa::path(
    get,
    path = "/registry/admin/tokens",
    responses(
        (status = 200, description = "List of tokens retrieved successfully"),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_tokens(
    AuthenticatedUser(_claims): AuthenticatedUser,
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
#[utoipa::path(
    post,
    path = "/registry/agents/register",
    request_body = RegisterAgentRequest,
    responses(
        (status = 200, description = "Agent registered successfully", body = RegisterAgentResponse),
        (status = 401, description = "Invalid registration token", body = ErrorResponse),
        (status = 403, description = "Agent name or hostname mismatch", body = ErrorResponse),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Agent"
)]
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
#[utoipa::path(
    post,
    path = "/registry/agents/{id}/heartbeat",
    params(
        ("id" = String, Path, description = "Agent ID")
    ),
    request_body = HeartbeatRequest,
    responses(
        (status = 200, description = "Heartbeat recorded successfully"),
        (status = 401, description = "Invalid or missing API key", body = ErrorResponse),
        (status = 403, description = "Agent ID mismatch", body = ErrorResponse),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Agent",
    security(
        ("api_key" = [])
    )
)]
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
#[utoipa::path(
    get,
    path = "/registry/admin/agents",
    responses(
        (status = 200, description = "List of agents retrieved successfully"),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_agents_admin(
    AuthenticatedUser(_claims): AuthenticatedUser,
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
#[utoipa::path(
    get,
    path = "/registry/admin/statistics",
    responses(
        (status = 200, description = "Statistics retrieved successfully", body = AgentStatistics),
        (status = 502, description = "Registry service unavailable", body = ErrorResponse)
    ),
    tag = "Registry - Admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_statistics(
    AuthenticatedUser(_claims): AuthenticatedUser,
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

/// Revoke agent certificate (Admin only)
pub async fn revoke_certificate(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    proxy_to_registry(
        &state,
        reqwest::Method::POST,
        &format!("/admin/agents/{}/revoke", agent_id),
        body_json,
        Some(header_map),
    )
    .await
}

/// Get agent mTLS endpoint info (Admin only)
pub async fn get_agent_endpoint(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    proxy_to_registry(
        &state,
        reqwest::Method::GET,
        &format!("/admin/agents/{}/endpoint", agent_id),
        None,
        Some(header_map),
    )
    .await
}

/// Get CRL (public)
pub async fn get_crl(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    proxy_to_registry(&state, reqwest::Method::GET, "/pki/crl", None, None).await
}

/// Issue certificate for agent
pub async fn issue_certificate(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    proxy_to_registry(
        &state,
        reqwest::Method::POST,
        &format!("/agents/{}/certificate", agent_id),
        body_json,
        Some(header_map),
    )
    .await
}

/// Rotate agent certificate
pub async fn rotate_certificate(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    proxy_to_registry(
        &state,
        reqwest::Method::POST,
        &format!("/agents/{}/rotate-certificate", agent_id),
        body_json,
        Some(header_map),
    )
    .await
}

async fn proxy_to_registry(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    headers: Option<Vec<(String, String)>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state
        .service_client
        .forward_to_registry(method, path, body, headers)
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

pub async fn create_bootstrap_token(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    proxy_to_registry(&state, reqwest::Method::POST, "/admin/bootstrap-tokens", body_json, Some(header_map)).await
}

pub async fn list_bootstrap_tokens(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    proxy_to_registry(&state, reqwest::Method::GET, "/admin/bootstrap-tokens", None, Some(header_map)).await
}

pub async fn revoke_bootstrap_token(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    proxy_to_registry(&state, reqwest::Method::POST, &format!("/admin/bootstrap-tokens/{}/revoke", id), None, Some(header_map)).await
}

/// Registry health check
#[utoipa::path(
    get,
    path = "/registry/health",
    responses(
        (status = 200, description = "Registry service is healthy"),
        (status = 503, description = "Registry service is unavailable")
    ),
    tag = "Registry - Health"
)]
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
        .route(
            "/registry/admin/agents/pre-register",
            post(pre_register_agent),
        )
        .route("/registry/admin/agents/pending", get(list_pending_agents))
        .route(
            "/registry/admin/agents/pending/:id",
            post(delete_pending_agent),
        )
        // Admin routes - Bootstrap Token Management
        .route("/registry/admin/bootstrap-tokens", post(create_bootstrap_token))
        .route("/registry/admin/bootstrap-tokens", get(list_bootstrap_tokens))
        .route("/registry/admin/bootstrap-tokens/:id/revoke", post(revoke_bootstrap_token))
        // Admin routes - Token Management (DEPRECATED)
        .route("/registry/admin/tokens", post(create_token))
        .route("/registry/admin/tokens", get(list_tokens))
        // Admin routes - Agent Management
        .route("/registry/admin/agents", get(list_agents_admin))
        .route("/registry/admin/statistics", get(get_statistics))
        // Admin routes - PKI
        .route(
            "/registry/admin/agents/:id/revoke",
            post(revoke_certificate),
        )
        .route(
            "/registry/admin/agents/:id/endpoint",
            get(get_agent_endpoint),
        )
        // Public PKI
        .route("/registry/pki/crl", get(get_crl))
        // Agent routes
        .route("/registry/agents/register", post(register_agent))
        .route("/registry/agents/:id/heartbeat", post(agent_heartbeat))
        // Agent - certificate management
        .route(
            "/registry/agents/:id/certificate",
            post(issue_certificate),
        )
        .route(
            "/registry/agents/:id/rotate-certificate",
            post(rotate_certificate),
        )
        // Health check
        .route("/registry/health", get(registry_health))
}
