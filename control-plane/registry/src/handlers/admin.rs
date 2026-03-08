use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    models::agent::{ErrorResponse, PreRegisterRequest, PreRegisterResponse},
    server::AppState,
    services::registry::PreRegisterParams,
};

#[derive(Debug, Deserialize)]
pub struct CreateBootstrapTokenRequest {
    pub description: Option<String>,
    pub ttl_hours: Option<i64>,
    pub max_uses: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct RevokeBootstrapTokenResponse {
    pub message: String,
}

pub async fn pre_register_agent(
    State(state): State<AppState>,
    Json(request): Json<PreRegisterRequest>,
) -> Result<Json<PreRegisterResponse>, (StatusCode, Json<ErrorResponse>)> {
    let created_by = "admin".to_string();
    let ttl_hours = request.ttl_hours.unwrap_or(24);
    let agent_id = Uuid::new_v4();

    let token = state
        .token_manager
        .create_token(
            agent_id,
            request.name.clone(),
            request.hostname.clone(),
            Some(format!(
                "Pre-registration for {}@{}",
                request.name, request.hostname
            )),
            created_by.clone(),
            ttl_hours,
        )
        .await;

    let pre_agent = state
        .agent_registry
        .pre_register_agent(PreRegisterParams {
            agent_id,
            name: request.name,
            hostname: request.hostname,
            expected_os_type: request.expected_os_type,
            expected_architecture: request.expected_architecture,
            tags: request.tags,
            created_by,
            registration_token: token.token.clone(),
            token_expires_at: token.expires_at,
        })
        .await;

    Ok(Json(PreRegisterResponse {
        agent_id: pre_agent.id,
        registration_token: token.token,
        token_expires_at: token.expires_at.to_rfc3339(),
        message: format!(
            "Agent '{}@{}' pre-registered. Use the registration token for initial connection.",
            pre_agent.name, pre_agent.hostname
        ),
    }))
}

pub async fn list_pending_agents(
    State(state): State<AppState>,
) -> Json<Vec<crate::models::agent::PreRegisteredAgent>> {
    Json(state.agent_registry.list_pending_agents().await)
}

pub async fn delete_pending_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state
        .agent_registry
        .delete_pre_registered_agent(agent_id)
        .await
    {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: e }),
        )),
    }
}

pub async fn list_tokens(
    State(state): State<AppState>,
) -> Json<Vec<crate::services::tokens::RegistrationToken>> {
    Json(state.token_manager.list_tokens().await)
}

pub async fn list_agents(
    State(state): State<AppState>,
) -> Json<Vec<crate::models::agent::RegisteredAgent>> {
    Json(state.agent_registry.list_agents().await)
}

pub async fn get_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<crate::models::agent::RegisteredAgent>, StatusCode> {
    match state.agent_registry.get_agent(agent_id).await {
        Some(agent) => Ok(Json(agent)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn deregister_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let _ = state.api_key_manager.revoke_key(agent_id).await;

    match state.agent_registry.deregister_agent(agent_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Failed to deregister agent: {}", e),
            }),
        )),
    }
}

pub async fn get_statistics(
    State(state): State<AppState>,
) -> Json<crate::models::agent::AgentStatistics> {
    Json(state.agent_registry.get_statistics().await)
}

pub async fn create_bootstrap_token(
    State(state): State<AppState>,
    Json(request): Json<CreateBootstrapTokenRequest>,
) -> Result<Json<crate::services::bootstrap_tokens::BootstrapToken>, (StatusCode, Json<ErrorResponse>)> {
    let ttl_hours = request.ttl_hours.unwrap_or(24 * 30);
    let max_uses = request.max_uses.unwrap_or(100);

    state
        .bootstrap_token_manager
        .create(request.description, "admin".to_string(), ttl_hours, max_uses)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e })))
}

pub async fn list_bootstrap_tokens(
    State(state): State<AppState>,
) -> Json<Vec<crate::services::bootstrap_tokens::BootstrapToken>> {
    Json(state.bootstrap_token_manager.list().await)
}

pub async fn revoke_bootstrap_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RevokeBootstrapTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .bootstrap_token_manager
        .revoke(id)
        .await
        .map(|_| Json(RevokeBootstrapTokenResponse { message: format!("Bootstrap token {} revoked", id) }))
        .map_err(|e| (StatusCode::NOT_FOUND, Json(ErrorResponse { error: e })))
}
