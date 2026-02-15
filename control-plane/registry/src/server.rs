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
pub struct PreRegisterAgentRequest {
    pub name: String,
    pub hostname: String,
    pub expected_os_type: Option<String>,
    pub expected_architecture: Option<String>,
    pub tags: Option<std::collections::HashMap<String, String>>,
    pub ttl_hours: Option<i64>, // Token TTL (default: 24h)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreRegisterAgentResponse {
    pub agent_id: Uuid,
    pub registration_token: String,
    pub token_expires_at: String,
    pub message: String,
}

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

/// Admin: Agent Pre-Registrieren (mit eigenem Token)
pub async fn pre_register_agent(
    State(state): State<AppState>,
    Json(request): Json<PreRegisterAgentRequest>,
) -> Result<Json<PreRegisterAgentResponse>, (StatusCode, Json<ErrorResponse>)> {
    // TODO: Extract from JWT/Authentication
    let created_by = "admin".to_string(); // Placeholder - in Phase 1 durch echte Auth ersetzen

    let ttl_hours = request.ttl_hours.unwrap_or(24);

    // Erstelle Token für diesen spezifischen Agent
    let token = state
        .token_manager
        .create_token(
            Uuid::new_v4(), // Wird gleich durch pre_register_agent überschrieben
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

    // Pre-Registriere Agent mit Token-ID als agent_id
    let pre_agent = state
        .agent_registry
        .pre_register_agent(crate::registry::PreRegisterParams {
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

    Ok(Json(PreRegisterAgentResponse {
        agent_id: pre_agent.id,
        registration_token: token.token,
        token_expires_at: token.expires_at.to_rfc3339(),
        message: format!(
            "Agent '{}@{}' pre-registered successfully. Use the registration token for initial connection.",
            pre_agent.name, pre_agent.hostname
        ),
    }))
}

/// Admin: Alle pending (pre-registrierten) Agents auflisten
pub async fn list_pending_agents(
    State(state): State<AppState>,
) -> Json<Vec<crate::registry::PreRegisteredAgent>> {
    let agents = state.agent_registry.list_pending_agents().await;
    Json(agents)
}

/// Admin: Pre-registrierten Agent löschen
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
            Json(ErrorResponse {
                error: format!("Failed to delete pre-registered agent: {}", e),
            }),
        )),
    }
}

/// Admin: Token erstellen (DEPRECATED - use pre_register_agent instead)
#[deprecated(note = "Use pre_register_agent for agent-specific tokens")]
pub async fn create_token(
    State(_state): State<AppState>,
    Json(_request): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Diese Funktion ist deprecated - Clients sollten pre_register_agent verwenden
    Err((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "This endpoint is deprecated. Please use POST /admin/agents/pre-register to create agent-specific registration tokens.".to_string(),
        }),
    ))
}

/// Admin: Alle Tokens auflisten
pub async fn list_tokens(
    State(state): State<AppState>,
) -> Json<Vec<crate::tokens::RegistrationToken>> {
    let tokens = state.token_manager.list_tokens().await;
    Json(tokens)
}

/// Agent: Registrierung mit Token (mit Pre-Registration Validierung)
pub async fn register_agent(
    State(state): State<AppState>,
    Json(request): Json<RegisterAgentRequest>,
) -> Result<Json<RegisterAgentResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validiere Token und hole Token-Daten
    let token_data = match state
        .token_manager
        .validate_and_consume_token(&request.registration_token)
        .await
    {
        Ok(token) => token,
        Err(e) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: format!("Invalid registration token: {}", e),
                }),
            ))
        }
    };

    // Validiere dass Name und Hostname mit erwarteten Werten übereinstimmen
    if token_data.expected_name != request.name {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: format!(
                    "Agent name mismatch. Expected '{}', got '{}'",
                    token_data.expected_name, request.name
                ),
            }),
        ));
    }

    if token_data.expected_hostname != request.hostname {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: format!(
                    "Agent hostname mismatch. Expected '{}', got '{}'",
                    token_data.expected_hostname, request.hostname
                ),
            }),
        ));
    }

    // Registriere Agent mit der agent_id aus dem Token
    let agent = match state
        .agent_registry
        .register_agent(crate::registry::RegisterAgentParams {
            agent_id: token_data.agent_id,
            name: request.name,
            hostname: request.hostname,
            os_type: request.os_type,
            os_version: request.os_version,
            architecture: request.architecture,
            agent_version: request.agent_version,
            tags: request.tags,
        })
        .await
    {
        Ok(agent) => agent,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to register agent: {}", e),
                }),
            ))
        }
    };

    // Erstelle API Key
    let api_key = state.api_key_manager.create_key(agent.id).await;

    Ok(Json(RegisterAgentResponse {
        agent_id: agent.id,
        api_key: api_key.key,
        message: "Agent successfully registered".to_string(),
    }))
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
pub async fn list_agents(
    State(state): State<AppState>,
) -> Json<Vec<crate::registry::RegisteredAgent>> {
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
pub async fn get_statistics(
    State(state): State<AppState>,
) -> Json<crate::registry::AgentStatistics> {
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
        // Admin Routes - Pre-Registration (NEW WORKFLOW)
        .route("/admin/agents/pre-register", post(pre_register_agent))
        .route("/admin/agents/pending", get(list_pending_agents))
        .route(
            "/admin/agents/pending/:agent_id",
            post(delete_pending_agent),
        )
        // Admin Routes - Token Management (DEPRECATED)
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
