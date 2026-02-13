use crate::{api_keys::ApiKeyManager, registry::AgentRegistry, tokens::TokenManager};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub token_manager: Arc<TokenManager>,
    pub api_key_manager: Arc<ApiKeyManager>,
    pub agent_registry: Arc<AgentRegistry>,
}

// ==================== Request/Response DTOs ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTokenRequest {
    pub description: Option<String>,
    pub created_by: String,
    pub ttl_hours: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTokenResponse {
    pub token_id: Uuid,
    pub token: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterAgentRequest {
    pub registration_token: String,
    pub name: String,
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub tags: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterAgentResponse {
    pub agent_id: Uuid,
    pub api_key: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ==================== API Routes ====================

/// Admin: Token erstellen
pub async fn create_token(
    State(state): State<AppState>,
    Json(request): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, StatusCode> {
    let token = state
        .token_manager
        .create_token(request.description, request.created_by, request.ttl_hours)
        .await;

    Ok(Json(CreateTokenResponse {
        token_id: token.id,
        token: token.token,
        expires_at: token.expires_at.to_rfc3339(),
    }))
}

/// Admin: Alle Tokens auflisten
pub async fn list_tokens(State(state): State<AppState>) -> Json<Vec<crate::tokens::RegistrationToken>> {
    let tokens = state.token_manager.list_tokens().await;
    Json(tokens)
}

/// Agent: Registrierung mit Token
pub async fn register_agent(
    State(state): State<AppState>,
    Json(request): Json<RegisterAgentRequest>,
) -> Result<Json<RegisterAgentResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validiere Token
    match state
        .token_manager
        .validate_and_consume_token(&request.registration_token)
        .await
    {
        Ok(_) => {
            // Registriere Agent
            let agent = state
                .agent_registry
                .register_agent(
                    request.name,
                    request.hostname,
                    request.os_type,
                    request.os_version,
                    request.architecture,
                    request.agent_version,
                    request.tags,
                )
                .await;

            // Erstelle API Key
            let api_key = state.api_key_manager.create_key(agent.id).await;

            Ok(Json(RegisterAgentResponse {
                agent_id: agent.id,
                api_key: api_key.key,
                message: "Agent successfully registered".to_string(),
            }))
        }
        Err(e) => Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: format!("Invalid registration token: {}", e),
            }),
        )),
    }
}

/// Agent: Heartbeat senden
pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(_request): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Extrahiere API Key aus Header
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Missing X-API-Key header".to_string(),
                }),
            )
        })?;

    // Validiere API Key
    match state.api_key_manager.validate_key(api_key).await {
        Ok(validated_agent_id) => {
            // Prüfe ob Agent ID übereinstimmt
            if validated_agent_id != agent_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "Agent ID mismatch".to_string(),
                    }),
                ));
            }

            // Update Heartbeat
            match state.agent_registry.update_heartbeat(agent_id).await {
                Ok(_) => Ok(Json(HeartbeatResponse {
                    success: true,
                    message: "Heartbeat recorded".to_string(),
                })),
                Err(e) => Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("Agent not found: {}", e),
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: format!("Invalid API key: {}", e),
            }),
        )),
    }
}

/// Admin: Alle Agents auflisten
pub async fn list_agents(State(state): State<AppState>) -> Json<Vec<crate::registry::RegisteredAgent>> {
    let agents = state.agent_registry.list_agents().await;
    Json(agents)
}

/// Admin: Agent Details
pub async fn get_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<crate::registry::RegisteredAgent>, StatusCode> {
    match state.agent_registry.get_agent(agent_id).await {
        Some(agent) => Ok(Json(agent)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Admin: Agent deregistrieren
pub async fn deregister_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Revoke API Key
    let _ = state.api_key_manager.revoke_key(agent_id).await;

    // Deregister agent
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

/// Admin: Agent Statistiken
pub async fn get_statistics(State(state): State<AppState>) -> Json<crate::registry::AgentStatistics> {
    let stats = state.agent_registry.get_statistics().await;
    Json(stats)
}

/// Health Check
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Registry Service OK")
}

// ==================== Router ====================

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health
        .route("/health", get(health_check))
        // Admin Routes - Token Management
        .route("/admin/tokens", post(create_token))
        .route("/admin/tokens", get(list_tokens))
        // Admin Routes - Agent Management
        .route("/admin/agents", get(list_agents))
        .route("/admin/agents/:agent_id", get(get_agent))
        .route("/admin/agents/:agent_id", post(deregister_agent))
        .route("/admin/statistics", get(get_statistics))
        // Agent Routes
        .route("/agents/register", post(register_agent))
        .route("/agents/:agent_id/heartbeat", post(heartbeat))
        .with_state(state)
}
