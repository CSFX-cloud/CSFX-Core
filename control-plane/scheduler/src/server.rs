use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    handlers::{internal, stacks, workloads},
    services::scheduler::SchedulerService,
};

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
    let router = Router::new()
        .route("/health", get(health_check))
        .route(
            "/workloads",
            axum::routing::post(workloads::create_workload),
        )
        .route("/workloads", get(workloads::list_workloads))
        .route(
            "/workloads/{id}",
            axum::routing::delete(workloads::delete_workload),
        )
        .route(
            "/workloads/{id}",
            axum::routing::patch(workloads::update_workload),
        )
        .route(
            "/workloads/{id}/stop",
            axum::routing::post(workloads::stop_workload),
        )
        .route(
            "/workloads/{id}/restart",
            axum::routing::post(workloads::restart_workload),
        )
        .route(
            "/workload-stacks",
            axum::routing::post(stacks::create_stack),
        )
        .route(
            "/workload-stacks/{id}",
            axum::routing::get(stacks::get_stack),
        )
        .route(
            "/workload-stacks/{id}",
            axum::routing::delete(stacks::delete_stack),
        )
        .route(
            "/workload-stacks/{id}/stop",
            axum::routing::post(stacks::stop_stack),
        )
        .route(
            "/workload-stacks/{id}/restart",
            axum::routing::post(stacks::restart_stack),
        )
        .route(
            "/workload-stacks/{id}",
            axum::routing::patch(stacks::redeploy_stack),
        )
        .route(
            "/internal/workloads/status",
            axum::routing::post(internal::update_container_statuses),
        )
        .route(
            "/internal/workloads/stats",
            axum::routing::post(internal::update_workload_stats),
        )
        .route(
            "/internal/workloads/{id}/restart-ack",
            axum::routing::post(internal::ack_workload_restart),
        )
        .route(
            "/internal/agents/{id}/reschedule",
            axum::routing::post(internal::reschedule_agent_workloads),
        )
        .with_state(state);
    shared::observability::instrument(router)
}
