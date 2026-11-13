use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    models::agent::{
        ErrorResponse, HeartbeatRequest, HeartbeatResponse, RegisterRequest, RegisterResponse,
        SignedRelease,
    },
    server::AppState,
    services::registry::RegisterAgentParams,
};

const VIP_ADDRESS_VAR: &str = "CSFX_VIP_ADDRESS";
const GATEWAY_PORT: u16 = 8000;
const WORKER_MANIFEST_PREFIX: &str = "/csfx/config/worker_release_manifest/";
const WORKER_SIGNATURE_PREFIX: &str = "/csfx/config/worker_release_manifest_sig/";

pub async fn register_agent(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<ErrorResponse>)> {
    let (agent_id, allow_reregister, role) =
        if crate::services::bootstrap_tokens::BootstrapTokenManager::is_bootstrap_token(
            &request.registration_token,
        ) {
            let role = state
                .bootstrap_token_manager
                .validate_and_use(&request.registration_token)
                .await
                .map_err(|e| {
                    (
                        StatusCode::UNAUTHORIZED,
                        Json(ErrorResponse {
                            error: format!("Invalid bootstrap token: {}", e),
                        }),
                    )
                })?;
            (uuid::Uuid::new_v4(), true, role)
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

            (
                token_data.agent_id,
                false,
                crate::services::bootstrap_tokens::ROLE_WORKER.to_string(),
            )
        };

    let csr_pem = request.csr_pem.clone();
    let proven_agent_id =
        proven_agent_id(&state, request.machine_id.as_deref(), csr_pem.as_deref()).await?;

    let (agent, reregistered) = match state
        .agent_registry
        .register_agent(
            RegisterAgentParams {
                agent_id,
                name: request.name,
                hostname: request.hostname,
                os_type: request.os_type,
                os_version: request.os_version,
                architecture: request.architecture,
                agent_version: request.agent_version,
                tags: request.tags,
                allow_reregister,
                proven_agent_id,
                role,
                machine_id: request.machine_id,
            },
            &state.mgmt_ipam,
        )
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
                &format!(
                    "Failed to revoke old API keys for agent={}: {}",
                    agent.id, e
                )
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
        wg_tunnel_ip: agent.wg_tunnel_ip,
        message: "Agent successfully registered".to_string(),
    }))
}

async fn proven_agent_id(
    state: &AppState,
    machine_id: Option<&str>,
    csr_pem: Option<&str>,
) -> Result<Option<Uuid>, (StatusCode, Json<ErrorResponse>)> {
    let (Some(machine_id), Some(csr_pem)) = (machine_id, csr_pem) else {
        return Ok(None);
    };
    let internal = |e: &dyn std::fmt::Display| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to verify agent identity: {}", e),
            }),
        )
    };
    let Some(owner) = crate::db::agents::get_by_machine_id(&state.db, machine_id)
        .await
        .map_err(|e| internal(&e))?
    else {
        return Ok(None);
    };
    let proven =
        crate::services::identity_proof::csr_proves_agent_identity(&state.db, owner.id, csr_pem)
            .await
            .map_err(|e| internal(&e))?;
    Ok(proven.then_some(owner.id))
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
                e.status(),
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

    if let Some(wg_endpoint) = request.wg_endpoint.as_deref() {
        let in_use = crate::db::agents::wg_endpoint_in_use(&state.db, agent_id, wg_endpoint)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Endpoint verification failed: {}", e),
                    }),
                )
            })?;

        if in_use {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "WireGuard endpoint is already in use by another agent".to_string(),
                }),
            ));
        }
    }

    match state
        .agent_registry
        .update_heartbeat(
            agent_id,
            request.wg_public_key.clone(),
            request.wg_endpoint.clone(),
            request.agent_version.clone(),
            request.kvm_capable,
            accepted_host_key(request.ssh_host_key.as_deref()),
            &state.mgmt_ipam,
        )
        .await
    {
        Ok(_) => {
            forward_metrics(&state, agent_id, &request).await;

            if let Some(statuses) = request.container_statuses {
                if !statuses.is_empty() {
                    forward_container_statuses(&state, agent_id, statuses).await;
                }
            }

            let worker_releases = read_worker_releases(&state.etcd_endpoints).await;

            tracing::info!(
                agent_id = %agent_id,
                releases = worker_releases.len(),
                "heartbeat processed"
            );

            Ok(Json(HeartbeatResponse {
                success: true,
                message: "Heartbeat recorded".to_string(),
                worker_releases,
                ssh_enabled: read_ssh_enabled(&state).await,
                gateway_url: advertised_gateway_url(std::env::var(VIP_ADDRESS_VAR).ok().as_deref()),
            }))
        }
        Err(e) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("Heartbeat could not be recorded: {}", e),
            }),
        )),
    }
}

fn accepted_host_key(raw: Option<&str>) -> Option<String> {
    let key = crate::services::ssh_host_key::normalize(raw?);
    if key.is_none() {
        crate::log_warn!(
            "agent_handler",
            "Ignoring malformed ssh host key in heartbeat"
        );
    }
    key
}

fn advertised_gateway_url(vip_address: Option<&str>) -> Option<String> {
    vip_address
        .filter(|address| !address.is_empty())
        .map(|address| format!("https://{address}:{GATEWAY_PORT}"))
}

async fn read_ssh_enabled(state: &AppState) -> bool {
    match shared::ssh_access::load(&state.db).await {
        Ok(access) => access.is_open(),
        Err(err) => {
            tracing::warn!(error = %err, "failed to read ssh access setting, keeping ssh closed");
            false
        }
    }
}

async fn read_worker_releases(etcd_endpoints: &str) -> HashMap<String, SignedRelease> {
    let mut client = match etcd_client::Client::connect([etcd_endpoints], None).await {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(error = %error, "etcd unreachable while reading worker releases");
            return HashMap::new();
        }
    };
    let manifests = read_prefix(&mut client, WORKER_MANIFEST_PREFIX).await;
    let mut signatures = read_prefix(&mut client, WORKER_SIGNATURE_PREFIX).await;
    manifests
        .into_iter()
        .filter_map(|(arch, manifest)| {
            signatures.remove(&arch).map(|signature| {
                (
                    arch,
                    SignedRelease {
                        manifest,
                        signature,
                    },
                )
            })
        })
        .collect()
}

async fn read_prefix(client: &mut etcd_client::Client, prefix: &str) -> HashMap<String, String> {
    let options = etcd_client::GetOptions::new().with_prefix();
    let response = match client.get(prefix, Some(options)).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(error = %error, prefix, "failed to read etcd prefix");
            return HashMap::new();
        }
    };
    response
        .kvs()
        .iter()
        .filter_map(|kv| {
            let key = kv.key_str().ok()?.strip_prefix(prefix)?.to_string();
            Some((key, kv.value_str().ok()?.to_string()))
        })
        .collect()
}

async fn forward_container_statuses(
    state: &crate::server::AppState,
    agent_id: Uuid,
    statuses: Vec<crate::models::agent::ContainerStatus>,
) {
    use serde_json::json;

    let payload = json!({
        "agent_id": agent_id,
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
            &format!(
                "Failed to forward container statuses to scheduler err={}",
                e
            )
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

    for attempt in 1..=2 {
        match state.http_client.post(&url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => return,
            Ok(resp) => {
                crate::log_warn!(
                    "agent_handler",
                    &format!(
                        "Metrics forward rejected by gateway agent_id={} attempt={} status={}",
                        agent_id,
                        attempt,
                        resp.status()
                    )
                );
            }
            Err(e) => {
                crate::log_warn!(
                    "agent_handler",
                    &format!(
                        "Failed to forward metrics to gateway agent_id={} attempt={} err={}",
                        agent_id, attempt, e
                    )
                );
            }
        }

        if attempt == 1 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    crate::log_error!(
        "agent_handler",
        &format!("Metrics forward permanently failed agent_id={}", agent_id)
    );
}

#[cfg(test)]
mod gateway_url_tests {
    use super::advertised_gateway_url;

    #[test]
    fn advertises_the_vip_when_configured() {
        assert_eq!(
            advertised_gateway_url(Some("192.168.1.50")).as_deref(),
            Some("https://192.168.1.50:8000")
        );
    }

    #[test]
    fn advertises_nothing_without_a_vip() {
        assert!(advertised_gateway_url(None).is_none());
        assert!(advertised_gateway_url(Some("")).is_none());
    }
}
