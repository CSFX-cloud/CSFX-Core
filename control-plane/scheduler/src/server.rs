use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub etcd: Arc<Mutex<EtcdClient>>,
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Scheduler Service OK")
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .with_state(state)
}
