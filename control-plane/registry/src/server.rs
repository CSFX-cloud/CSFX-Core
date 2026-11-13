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
    services::{
        api_keys::ApiKeyManager, bootstrap_tokens::BootstrapTokenManager,
        mgmt_ipam::MgmtIpamService, pki::PkiService, registry::AgentRegistry, tokens::TokenManager,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub token_manager: Arc<TokenManager>,
    pub bootstrap_token_manager: Arc<BootstrapTokenManager>,
    pub api_key_manager: Arc<ApiKeyManager>,
    pub agent_registry: Arc<AgentRegistry>,
    pub pki_service: Arc<PkiService>,
    pub mgmt_ipam: Arc<MgmtIpamService>,
    pub db: DatabaseConnection,
    pub scheduler_url: String,
    pub gateway_url: String,
    pub http_client: Client,
    pub etcd_endpoints: String,
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Registry Service OK")
}

pub fn create_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/health", get(health_check))
        .route(
            "/admin/agents/pre-register",
            post(admin::pre_register_agent),
        )
        .route("/admin/agents/pending", get(admin::list_pending_agents))
        .route(
            "/admin/agents/pending/{agent_id}",
            delete(admin::delete_pending_agent),
        )
        .route("/admin/tokens", get(admin::list_tokens))
        .route(
            "/admin/bootstrap-tokens",
            post(admin::create_bootstrap_token),
        )
        .route("/admin/bootstrap-tokens", get(admin::list_bootstrap_tokens))
        .route(
            "/admin/bootstrap-tokens/{id}/revoke",
            post(admin::revoke_bootstrap_token),
        )
        .route("/admin/agents", get(admin::list_agents))
        .route("/admin/agents/{agent_id}", get(admin::get_agent))
        .route("/admin/agents/{agent_id}", delete(admin::deregister_agent))
        .route("/admin/statistics", get(admin::get_statistics))
        .route(
            "/admin/agents/{agent_id}/revoke",
            post(pki::revoke_certificate),
        )
        .route(
            "/admin/agents/{agent_id}/endpoint",
            get(pki::get_agent_endpoint),
        )
        .route("/pki/crl", get(pki::get_crl))
        .route(
            "/internal/agent-proxy-ticket",
            post(pki::issue_proxy_ticket),
        )
        .route("/agents/register", post(agent::register_agent))
        .route("/agents/{agent_id}/heartbeat", post(agent::heartbeat))
        .route(
            "/agents/{agent_id}/certificate",
            post(pki::issue_certificate),
        )
        .route(
            "/agents/{agent_id}/rotate-certificate",
            post(pki::rotate_certificate),
        )
        .with_state(state);
    shared::observability::instrument(router)
}
