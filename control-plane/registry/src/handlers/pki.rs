use axum::{
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::{
    models::agent::ErrorResponse,
    models::certificate::{
        AgentEndpointResponse, CrlResponse, IssueCertificateRequest, IssueCertificateResponse,
        RevokeCertificateRequest, RotateCertificateRequest,
    },
    server::AppState,
};

pub async fn issue_certificate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<IssueCertificateRequest>,
) -> Result<Json<IssueCertificateResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    let validated_id = state
        .api_key_manager
        .validate_key(api_key)
        .await
        .map_err(|e| {
            (
                e.status(),
                Json(ErrorResponse {
                    error: format!("Invalid API key: {}", e),
                }),
            )
        })?;

    if validated_id != agent_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Agent ID mismatch".to_string(),
            }),
        ));
    }

    let issued = state
        .pki_service
        .issue_certificate(agent_id, &request.csr_pem)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to issue certificate: {}", e),
                }),
            )
        })?;

    Ok(Json(IssueCertificateResponse {
        certificate_pem: issued.certificate_pem,
        ca_cert_pem: state.pki_service.ca_cert_pem(),
        serial_number: issued.serial_number,
        expires_at: issued.expires_at.to_rfc3339(),
    }))
}

pub async fn rotate_certificate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<RotateCertificateRequest>,
) -> Result<Json<IssueCertificateResponse>, (StatusCode, Json<ErrorResponse>)> {
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

    let validated_id = state
        .api_key_manager
        .validate_key(api_key)
        .await
        .map_err(|e| {
            (
                e.status(),
                Json(ErrorResponse {
                    error: format!("Invalid API key: {}", e),
                }),
            )
        })?;

    if validated_id != agent_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Agent ID mismatch".to_string(),
            }),
        ));
    }

    let issued = state
        .pki_service
        .rotate_certificate(agent_id, &request.new_csr_pem)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to rotate certificate: {}", e),
                }),
            )
        })?;

    Ok(Json(IssueCertificateResponse {
        certificate_pem: issued.certificate_pem,
        ca_cert_pem: state.pki_service.ca_cert_pem(),
        serial_number: issued.serial_number,
        expires_at: issued.expires_at.to_rfc3339(),
    }))
}

pub async fn revoke_certificate(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Json(request): Json<RevokeCertificateRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .pki_service
        .revoke_agent_certificate(agent_id, request.reason)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to revoke certificate: {}", e),
                }),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_crl(
    State(state): State<AppState>,
) -> Result<Json<CrlResponse>, (StatusCode, Json<ErrorResponse>)> {
    let revoked_serials = state.pki_service.build_crl().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to build CRL: {}", e),
            }),
        )
    })?;

    Ok(Json(CrlResponse {
        revoked_serials,
        generated_at: Utc::now(),
    }))
}

#[derive(Deserialize)]
pub struct ProxyTicketRequest {
    pub agent_id: Uuid,
    pub workload_id: String,
}

#[derive(Serialize)]
pub struct ProxyTicketResponse {
    pub payload: String,
    pub signature: String,
}

pub async fn issue_proxy_ticket(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(request): Json<ProxyTicketRequest>,
) -> Result<Json<ProxyTicketResponse>, (StatusCode, Json<ErrorResponse>)> {
    let ip = addr.ip();
    let allowed = ip.is_loopback()
        || match ip {
            std::net::IpAddr::V4(v4) => {
                let octets = v4.octets();
                octets[0] == 10
                    || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
                    || (octets[0] == 192 && octets[1] == 168)
            }
            std::net::IpAddr::V6(_) => false,
        };

    if !allowed {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "internal endpoint not accessible from this network".to_string(),
            }),
        ));
    }

    let ticket = state
        .pki_service
        .sign_proxy_ticket(request.agent_id, &request.workload_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to sign proxy ticket: {}", e),
                }),
            )
        })?;

    Ok(Json(ProxyTicketResponse {
        payload: ticket.payload,
        signature: ticket.signature,
    }))
}

pub async fn get_agent_endpoint(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentEndpointResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agent = state
        .agent_registry
        .get_agent(agent_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Agent not found: {}", agent_id),
                }),
            )
        })?;

    let public_key_pem = crate::db::certificates::get_active_certificate(&state.db, agent_id)
        .await
        .ok()
        .flatten()
        .map(|c| c.public_key_pem);

    Ok(Json(AgentEndpointResponse {
        agent_id,
        hostname: agent.hostname,
        ip_address: agent.ip_address,
        public_key_pem,
    }))
}
