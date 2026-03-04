use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use crate::{
    handlers::{admin, agent},
    services::{api_keys::ApiKeyManager, registry::AgentRegistry, tokens::TokenManager},
};

#[derive(Clone)]
pub struct AppState {
    pub token_manager: Arc<TokenManager>,
    pub api_key_manager: Arc<ApiKeyManager>,
    pub agent_registry: Arc<AgentRegistry>,
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Registry Service OK")
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
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
        .route("/agents/register", post(agent::register_agent))
        .route("/agents/:agent_id/heartbeat", post(agent::heartbeat))
        .with_state(state)
}
