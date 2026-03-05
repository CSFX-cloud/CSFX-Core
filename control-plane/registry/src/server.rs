use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use reqwest::Client;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::{
    handlers::{admin, agent, pki},
    services::{api_keys::ApiKeyManager, pki::PkiService, registry::AgentRegistry, tokens::TokenManager},
};

#[derive(Clone)]
pub struct AppState {
    pub token_manager: Arc<TokenManager>,
    pub api_key_manager: Arc<ApiKeyManager>,
    pub agent_registry: Arc<AgentRegistry>,
    pub pki_service: Arc<PkiService>,
    pub db: DatabaseConnection,
    pub scheduler_url: String,
    pub http_client: Client,
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Registry Service OK")
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        // Admin — agent lifecycle
        .route("/admin/agents/pre-register", post(admin::pre_register_agent))
        .route("/admin/agents/pending", get(admin::list_pending_agents))
        .route(
            "/admin/agents/pending/:agent_id",
            delete(admin::delete_pending_agent),
        )
        .route("/admin/tokens", get(admin::list_tokens))
        .route("/admin/agents", get(admin::list_agents))
        .route("/admin/agents/:agent_id", get(admin::get_agent))
        .route("/admin/agents/:agent_id", delete(admin::deregister_agent))
        .route("/admin/statistics", get(admin::get_statistics))
        // Admin — PKI
        .route(
            "/admin/agents/:agent_id/revoke",
            post(pki::revoke_certificate),
        )
        .route(
            "/admin/agents/:agent_id/endpoint",
            get(pki::get_agent_endpoint),
        )
        // Public PKI
        .route("/pki/crl", get(pki::get_crl))
        // Agent — registration + heartbeat
        .route("/agents/register", post(agent::register_agent))
        .route("/agents/:agent_id/heartbeat", post(agent::heartbeat))
        // Agent — certificate management
        .route(
            "/agents/:agent_id/certificate",
            post(pki::issue_certificate),
        )
        .route(
            "/agents/:agent_id/rotate-certificate",
            post(pki::rotate_certificate),
        )
        .with_state(state)
}
