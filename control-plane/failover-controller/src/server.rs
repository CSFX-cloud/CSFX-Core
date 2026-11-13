use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use sea_orm::DatabaseConnection;

use crate::handlers::events;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Failover Controller OK")
}

pub fn create_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/health", get(health_check))
        .route("/events", get(events::list_events))
        .route("/events/{event_id}/resolve", post(events::resolve_event))
        .route(
            "/agents/{agent_id}/maintenance",
            post(events::set_maintenance),
        )
        .route(
            "/agents/{agent_id}/maintenance",
            delete(events::clear_maintenance),
        )
        .with_state(state);
    shared::observability::instrument(router)
}
