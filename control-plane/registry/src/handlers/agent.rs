use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use csfx_agent_api::{
    ContainerStatus, HeartbeatRequest, HeartbeatResponse, RegisterRequest, RegisterResponse,
    SignedRelease,
};
use entity::agent_metrics;
use sea_orm::{ActiveModelTrait, Set};
use shared::internal_auth::SignedRequest;
use shared::ttl_cache::TtlCache;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::{
    db::agents::Heartbeat,
    handlers::{api_error, ApiError},
    server::AppState,
    services::{bootstrap_tokens::ROLE_WORKER, registry::RegisterAgentParams, worker_rollout},
};

const VIP_ADDRESS_VAR: &str = "CSFX_VIP_ADDRESS";
const GATEWAY_PORT: u16 = 8000;
const SSH_SETTING_TTL: Duration = Duration::from_secs(5);
const ENDPOINT_CHECK_TTL: Duration = Duration::from_secs(300);
const RELEASE_CACHE_TTL: Duration = Duration::from_secs(60);

type ReleaseCache = Mutex<Option<(Instant, HashMap<String, SignedRelease>)>>;

static SSH_SETTING: LazyLock<TtlCache<bool>> = LazyLock::new(|| TtlCache::new(SSH_SETTING_TTL));
static VERIFIED_ENDPOINTS: LazyLock<TtlCache<String>> =
    LazyLock::new(|| TtlCache::new(ENDPOINT_CHECK_TTL));

static RELEASE_CACHE: LazyLock<ReleaseCache> = LazyLock::new(|| Mutex::new(None));
use shared::etcd::{
    WORKER_MANIFEST_PREFIX, WORKER_MANIFEST_SIGNATURE_PREFIX as WORKER_SIGNATURE_PREFIX,
};

pub async fn register_agent(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    let (agent_id, allow_reregister, role) =
        if crate::services::bootstrap_tokens::BootstrapTokenManager::is_bootstrap_token(
            &request.registration_token,
        ) {
            let role = state
                .bootstrap_token_manager
                .validate_and_use(&request.registration_token)
                .await
                .map_err(|e| {
                    api_error(
                        StatusCode::UNAUTHORIZED,
                        format!("Invalid bootstrap token: {}", e),
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
                    return Err(api_error(
                        StatusCode::UNAUTHORIZED,
                        format!("Invalid registration token: {}", e),
                    ))
                }
            };

            if token_data.expected_name != request.name {
                return Err(api_error(
                    StatusCode::FORBIDDEN,
                    format!(
                        "Agent name mismatch. Expected '{}', got '{}'",
                        token_data.expected_name, request.name
                    ),
                ));
            }

            if token_data.expected_hostname != request.hostname {
                return Err(api_error(
                    StatusCode::FORBIDDEN,
                    format!(
                        "Agent hostname mismatch. Expected '{}', got '{}'",
                        token_data.expected_hostname, request.hostname
                    ),
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
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to register agent: {}", e),
            ))
        }
    };

    let api_key = match state
        .api_key_manager
        .issue_key(agent.id, reregistered)
        .await
    {
        Ok(api_key) => api_key,
        Err(e) => {
            crate::log_error!(
                "agent_handler",
                &format!("Failed to issue API key for agent={}: {}", agent.id, e)
            );
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to issue API key",
            ));
        }
    };

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
) -> Result<Option<Uuid>, ApiError> {
    let (Some(machine_id), Some(csr_pem)) = (machine_id, csr_pem) else {
        return Ok(None);
    };
    let internal = |e: &dyn std::fmt::Display| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to verify agent identity: {}", e),
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
) -> Result<Json<HeartbeatResponse>, ApiError> {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Missing X-API-Key header"))?;

    let validated_agent_id = match state.api_key_manager.validate_key(api_key).await {
        Ok(id) => id,
        Err(e) => return Err(api_error(e.status(), format!("Invalid API key: {}", e))),
    };

    if validated_agent_id != agent_id {
        return Err(api_error(StatusCode::FORBIDDEN, "Agent ID mismatch"));
    }

    if let Some(cert_pem) = headers.get("X-Client-Cert").and_then(|v| v.to_str().ok()) {
        let valid = crate::db::certificates::verify_client_cert(&state.db, agent_id, cert_pem)
            .await
            .map_err(|e| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Certificate verification failed: {}", e),
                )
            })?;

        if !valid {
            return Err(api_error(
                StatusCode::FORBIDDEN,
                "Invalid or revoked client certificate",
            ));
        }
    }

    if let Some(wg_endpoint) = request.wg_endpoint.as_deref() {
        verify_wg_endpoint(&state, agent_id, wg_endpoint).await?;
    }

    match state
        .agent_registry
        .update_heartbeat(
            agent_id,
            Heartbeat {
                wg_public_key: request.wg_public_key.clone(),
                wg_endpoint: request.wg_endpoint.clone(),
                agent_version: request.agent_version.clone(),
                kvm_capable: request.kvm_capable,
                ssh_host_key: accepted_host_key(request.ssh_host_key.as_deref()),
            },
            &state.mgmt_ipam,
        )
        .await
    {
        Ok(role) => {
            store_metrics(&state, agent_id, &request).await;

            if let Some(statuses) = request.container_statuses {
                if !statuses.is_empty() {
                    forward_container_statuses(&state, agent_id, statuses).await;
                }
            }

            let worker_releases = read_worker_releases(
                &state,
                agent_id,
                &role,
                request.release_version.as_deref().unwrap_or_default(),
            )
            .await;

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
        Err(e) => Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Heartbeat could not be recorded: {}", e),
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

async fn verify_wg_endpoint(
    state: &AppState,
    agent_id: Uuid,
    wg_endpoint: &str,
) -> Result<(), ApiError> {
    let key = agent_id.to_string();
    if VERIFIED_ENDPOINTS.get(&key).as_deref() == Some(wg_endpoint) {
        return Ok(());
    }
    let in_use = crate::db::agents::wg_endpoint_in_use(&state.db, agent_id, wg_endpoint)
        .await
        .map_err(|e| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Endpoint verification failed: {}", e),
            )
        })?;
    if in_use {
        return Err(api_error(
            StatusCode::CONFLICT,
            "WireGuard endpoint is already in use by another agent",
        ));
    }
    VERIFIED_ENDPOINTS.insert(&key, wg_endpoint.to_string());
    Ok(())
}

async fn read_ssh_enabled(state: &AppState) -> bool {
    if let Some(open) = SSH_SETTING.get("open") {
        return open;
    }
    match shared::ssh_access::load(&state.db).await {
        Ok(access) => {
            SSH_SETTING.insert("open", access.is_open());
            access.is_open()
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to read ssh access setting, keeping ssh closed");
            false
        }
    }
}

async fn read_worker_releases(
    state: &AppState,
    agent_id: Uuid,
    role: &str,
    release_version: &str,
) -> HashMap<String, SignedRelease> {
    let mut client = match state.etcd_client().await {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(error = %error, "etcd unreachable while reading worker releases");
            return HashMap::new();
        }
    };
    if role == ROLE_WORKER {
        report_release(&mut client, agent_id, release_version).await;
        if !rollout_admits(&mut client, agent_id).await {
            return HashMap::new();
        }
    }
    cached_signed_releases(&mut client).await
}

async fn cached_signed_releases(
    client: &mut etcd_client::Client,
) -> HashMap<String, SignedRelease> {
    if let Some(releases) = fresh_cached_releases() {
        return releases;
    }
    let releases = read_signed_releases(client).await;
    if !releases.is_empty() {
        *RELEASE_CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now(), releases.clone()));
    }
    releases
}

fn fresh_cached_releases() -> Option<HashMap<String, SignedRelease>> {
    let cache = RELEASE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let (stored_at, releases) = cache.as_ref()?;
    (stored_at.elapsed() < RELEASE_CACHE_TTL).then(|| releases.clone())
}

async fn read_signed_releases(client: &mut etcd_client::Client) -> HashMap<String, SignedRelease> {
    let manifests = read_prefix(client, WORKER_MANIFEST_PREFIX).await;
    let mut signatures = read_prefix(client, WORKER_SIGNATURE_PREFIX).await;
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

async fn report_release(client: &mut etcd_client::Client, agent_id: Uuid, release_version: &str) {
    let seen = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let value = worker_rollout::fleet_value(seen, release_version);
    if let Err(error) = client
        .put(worker_rollout::fleet_key(agent_id), value, None)
        .await
    {
        tracing::warn!(error = %error, agent_id = %agent_id, "failed to report worker release");
    }
}

async fn rollout_admits(client: &mut etcd_client::Client, agent_id: Uuid) -> bool {
    let response = match client.get(worker_rollout::HOSTS_KEY, None).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(error = %error, "failed to read rollout host list, withholding release");
            return false;
        }
    };
    let hosts = response.kvs().first().and_then(|kv| kv.value_str().ok());
    worker_rollout::admits(hosts, agent_id)
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
    statuses: Vec<ContainerStatus>,
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

    if let Err(e) = state
        .http_client
        .post(&url)
        .signed()
        .json(&payload)
        .send()
        .await
    {
        crate::log_warn!(
            "agent_handler",
            &format!(
                "Failed to forward container statuses to scheduler err={}",
                e
            )
        );
    }
}

fn percent_of(used: u64, total: u64) -> Option<f32> {
    (total > 0).then(|| used as f32 / total as f32 * 100.0)
}

async fn store_metrics(
    state: &crate::server::AppState,
    agent_id: Uuid,
    request: &HeartbeatRequest,
) {
    let Some(metrics) = request.metrics.as_ref() else {
        return;
    };

    let row = agent_metrics::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        timestamp: Set(chrono::Utc::now().naive_utc()),
        cpu_usage_percent: Set(Some(metrics.cpu_usage_percent)),
        cpu_cores: Set(Some(metrics.cpu_cores as i32)),
        memory_total_bytes: Set(Some(metrics.memory_total_bytes as i64)),
        memory_used_bytes: Set(Some(metrics.memory_used_bytes as i64)),
        memory_usage_percent: Set(percent_of(
            metrics.memory_used_bytes,
            metrics.memory_total_bytes,
        )),
        disk_total_bytes: Set(Some(metrics.disk_total_bytes as i64)),
        disk_used_bytes: Set(Some(metrics.disk_used_bytes as i64)),
        disk_usage_percent: Set(percent_of(
            metrics.disk_used_bytes,
            metrics.disk_total_bytes,
        )),
        network_rx_bytes: Set(Some(metrics.network_rx_bytes as i64)),
        network_tx_bytes: Set(Some(metrics.network_tx_bytes as i64)),
        uptime_seconds: Set(Some(metrics.uptime_seconds as i64)),
        ..Default::default()
    };

    if let Err(e) = row.insert(&state.db).await {
        crate::log_error!(
            "agent_handler",
            &format!("Failed to store metrics agent_id={} err={}", agent_id, e)
        );
    }
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
