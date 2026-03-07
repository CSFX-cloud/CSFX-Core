use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{handlers::volumes, metrics, services::volume::VolumeService};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub etcd: Arc<Mutex<EtcdClient>>,
    pub volume_service: Arc<VolumeService>,
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Volume Manager OK")
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics::metrics_handler))
        .route("/volumes", axum::routing::post(volumes::create_volume))
        .route("/volumes", get(volumes::list_volumes))
        .route("/volumes/:id", get(volumes::get_volume))
        .route("/volumes/:id", axum::routing::delete(volumes::delete_volume))
        .route("/volumes/:id/attach", axum::routing::post(volumes::attach_volume))
        .route("/volumes/:id/detach", axum::routing::post(volumes::detach_volume))
        .route("/volumes/:id/snapshots", axum::routing::post(volumes::create_snapshot))
        .route("/volumes/:id/snapshots", get(volumes::list_snapshots))
        .with_state(state)
}
