use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
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
    Router::new()
        .route("/health", get(health_check))
        .route("/events", get(events::list_events))
        .with_state(state)
}
