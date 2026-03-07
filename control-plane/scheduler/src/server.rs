use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{handlers::{internal, workloads}, metrics, services::scheduler::SchedulerService};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub etcd: Arc<Mutex<EtcdClient>>,
    pub scheduler: Arc<SchedulerService>,
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Scheduler Service OK")
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics::metrics_handler))
        .route("/workloads", axum::routing::post(workloads::create_workload))
        .route("/workloads", get(workloads::list_workloads))
        .route("/workloads/:id", axum::routing::delete(workloads::delete_workload))
        .route("/internal/workloads/status", axum::routing::post(internal::update_container_statuses))
        .with_state(state)
}
