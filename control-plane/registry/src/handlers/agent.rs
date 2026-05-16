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
    let (agent_id, allow_reregister) =
        if crate::services::bootstrap_tokens::BootstrapTokenManager::is_bootstrap_token(
            &request.registration_token,
        ) {
            if let Err(e) = state
                .bootstrap_token_manager
                .validate_and_use(&request.registration_token)
                .await
            {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: format!("Invalid bootstrap token: {}", e),
                    }),
                ));
            }
            (uuid::Uuid::new_v4(), true)
        } else {
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

            (token_data.agent_id, false)
        };

    let csr_pem = request.csr_pem.clone();

    let (agent, reregistered) = match state
        .agent_registry
        .register_agent(RegisterAgentParams {
            agent_id,
            name: request.name,
            hostname: request.hostname,
            os_type: request.os_type,
            os_version: request.os_version,
            architecture: request.architecture,
            agent_version: request.agent_version,
            tags: request.tags,
            allow_reregister,
        })
        .await
    {
        Ok(result) => result,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to register agent: {}", e),
                }),
            ))
        }
    };

    if reregistered {
        if let Err(e) = state.api_key_manager.revoke_all_keys(agent.id).await {
            crate::log_warn!(
                "agent_handler",
                &format!("Failed to revoke old API keys for agent={}: {}", agent.id, e)
            );
        }
    }

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
            forward_metrics(&state, agent_id, &request).await;

            if let Some(statuses) = request.container_statuses {
                if !statuses.is_empty() {
                    forward_container_statuses(&state, statuses).await;
                }
            }

            let desired_flake_rev = read_desired_flake_rev(&state.etcd_endpoints).await;
            let post_update_heartbeats =
                increment_post_update_heartbeats(&state.etcd_endpoints, agent_id).await;

            tracing::info!(
                agent_id = %agent_id,
                desired_flake_rev = ?desired_flake_rev,
                post_update_heartbeats = ?post_update_heartbeats,
                "heartbeat processed"
            );

            Ok(Json(HeartbeatResponse {
                success: true,
                message: "Heartbeat recorded".to_string(),
                desired_flake_rev,
                post_update_heartbeats,
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

async fn read_desired_flake_rev(etcd_endpoints: &str) -> Option<String> {
    let mut client = etcd_client::Client::connect([etcd_endpoints], None)
        .await
        .ok()?;

    let resp = client
        .get("/csfx/config/desired_flake_rev", None)
        .await
        .ok()?;

    resp.kvs()
        .first()
        .and_then(|kv| std::str::from_utf8(kv.value()).ok())
        .map(|s| s.to_string())
}

async fn increment_post_update_heartbeats(etcd_endpoints: &str, agent_id: Uuid) -> Option<u32> {
    let key = format!("/csfx/nodes/{}/post_update_heartbeats", agent_id);

    let mut client = etcd_client::Client::connect([etcd_endpoints], None)
        .await
        .ok()?;

    let current: u32 = client
        .get(key.as_str(), None)
        .await
        .ok()
        .and_then(|r| r.kvs().first().map(|kv| kv.value().to_vec()))
        .and_then(|v| std::str::from_utf8(&v).ok().and_then(|s| s.parse().ok()))
        .unwrap_or(0);

    let next = current + 1;

    client
        .put(key.as_str(), next.to_string().as_bytes(), None)
        .await
        .ok()?;

    Some(next)
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

async fn forward_metrics(
    state: &crate::server::AppState,
    agent_id: Uuid,
    request: &HeartbeatRequest,
) {
    if request.cpu_usage_percent.is_none() && request.memory_total_bytes.is_none() {
        return;
    }

    use serde_json::json;

    let payload = json!({
        "agent_id": agent_id,
        "timestamp": chrono::Utc::now(),
        "cpu_usage_percent": request.cpu_usage_percent,
        "cpu_cores": request.cpu_cores,
        "memory_total_bytes": request.memory_total_bytes,
        "memory_used_bytes": request.memory_used_bytes,
        "disk_total_bytes": request.disk_total_bytes,
        "disk_used_bytes": request.disk_used_bytes,
        "network_rx_bytes": request.network_rx_bytes,
        "network_tx_bytes": request.network_tx_bytes,
        "uptime_seconds": request.uptime_seconds,
    });

    let url = format!("{}/api/agents/metrics", state.gateway_url);

    if let Err(e) = state.http_client.post(&url).json(&payload).send().await {
        crate::log_warn!(
            "agent_handler",
            &format!("Failed to forward metrics to gateway err={}", e)
        );
    }
}
