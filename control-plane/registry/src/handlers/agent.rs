use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use uuid::Uuid;

use crate::{
    models::agent::{ErrorResponse, HeartbeatRequest, HeartbeatResponse, RegisterRequest, RegisterResponse},
    server::AppState,
    services::registry::RegisterAgentParams,
};

pub async fn register_agent(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    let csr_pem = request.csr_pem.clone();

    let agent = match state
        .agent_registry
        .register_agent(RegisterAgentParams {
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

    let api_key = state.api_key_manager.create_key(agent.id).await;

    let (certificate_pem, ca_cert_pem) = if let Some(ref csr) = csr_pem {
        match state.pki_service.issue_certificate(agent.id, csr).await {
            Ok(issued) => (
                Some(issued.certificate_pem),
                Some(state.pki_service.ca_cert_pem()),
            ),
            Err(e) => {
                crate::log_warn!(
                    "agent_handler",
                    &format!("Failed to issue certificate during registration: {}", e)
                );
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    Ok(Json(RegisterResponse {
        agent_id: agent.id,
        api_key: api_key.key,
        certificate_pem,
        ca_cert_pem,
        message: "Agent successfully registered".to_string(),
    }))
}

pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    let validated_agent_id = match state.api_key_manager.validate_key(api_key).await {
        Ok(id) => id,
        Err(e) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: format!("Invalid API key: {}", e),
                }),
            ))
        }
    };

    if validated_agent_id != agent_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Agent ID mismatch".to_string(),
            }),
        ));
    }

    if let Some(cert_pem) = headers.get("X-Client-Cert").and_then(|v| v.to_str().ok()) {
        let valid = crate::db::certificates::verify_client_cert(&state.db, agent_id, cert_pem)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Certificate verification failed: {}", e),
                    }),
                )
            })?;

        if !valid {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Invalid or revoked client certificate".to_string(),
                }),
            ));
        }
    }

    match state.agent_registry.update_heartbeat(agent_id).await {
        Ok(_) => {
            if let Some(statuses) = request.container_statuses {
                if !statuses.is_empty() {
                    forward_container_statuses(&state, statuses).await;
                }
            }

            Ok(Json(HeartbeatResponse {
                success: true,
                message: "Heartbeat recorded".to_string(),
            }))
        }
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Agent not found: {}", e),
            }),
        )),
    }
}

async fn forward_container_statuses(
    state: &crate::server::AppState,
    statuses: Vec<crate::models::agent::ContainerStatus>,
) {
    use serde_json::json;

    let payload = json!({
        "statuses": statuses.iter().map(|s| json!({
            "workload_id": s.workload_id,
            "container_id": s.container_id,
            "status": s.status,
        })).collect::<Vec<_>>()
    });

    let url = format!("{}/internal/workloads/status", state.scheduler_url);

    if let Err(e) = state.http_client.post(&url).json(&payload).send().await {
        crate::log_warn!(
            "agent_handler",
            &format!("Failed to forward container statuses to scheduler err={}", e)
        );
    }
}
