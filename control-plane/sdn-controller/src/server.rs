use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use sea_orm::DatabaseConnection;

use crate::{
    handlers::networks,
    services::ipam::IpamService,
};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub ipam: IpamService,
}

impl AppState {
    pub fn new(db: DatabaseConnection, ipam: IpamService) -> Self {
        Self { db, ipam }
    }
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "SDN Controller OK")
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/networks", get(networks::list_networks).post(networks::create_network))
        .route(
            "/networks/:id",
            get(networks::get_network).delete(networks::delete_network),
        )
        .route(
            "/networks/:id/policies",
            get(networks::list_policies).post(networks::create_policy),
        )
        .route(
            "/networks/:id/members",
            get(networks::list_members).post(networks::add_member),
        )
        .route(
            "/networks/:id/members/:workload_id",
            axum::routing::delete(networks::remove_member),
        )
        .with_state(state)
}
