use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, Method},
    response::Response,
    routing::{delete, get},
    Router,
};

use crate::{
    auth::rbac::{CanManageNetworks, CanViewNetworks},
    routes::proxy::{proxy, ProxyError},
    service_client::Service,
    AppState,
};

async fn sdn(
    state: &AppState,
    method: Method,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    proxy(state, Service::SdnController, method, path, headers, body).await
}

pub async fn list_networks(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    sdn(&state, Method::GET, "/networks", &headers, Bytes::new()).await
}

pub async fn create_network(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    sdn(&state, Method::POST, "/networks", &headers, body).await
}

pub async fn get_network(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/networks/{id}");
    sdn(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn delete_network(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/networks/{id}");
    sdn(&state, Method::DELETE, &path, &headers, Bytes::new()).await
}

pub async fn list_policies(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/networks/{id}/policies");
    sdn(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn create_policy(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/networks/{id}/policies");
    sdn(&state, Method::POST, &path, &headers, body).await
}

pub async fn list_members(
    CanViewNetworks(_claims): CanViewNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/networks/{id}/members");
    sdn(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn add_member(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/networks/{id}/members");
    sdn(&state, Method::POST, &path, &headers, body).await
}

pub async fn remove_member(
    CanManageNetworks(_claims): CanManageNetworks,
    State(state): State<AppState>,
    Path((id, workload_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/networks/{id}/members/{workload_id}");
    sdn(&state, Method::DELETE, &path, &headers, Bytes::new()).await
}

pub fn networks_routes() -> Router<AppState> {
    Router::new()
        .route("/networks", get(list_networks).post(create_network))
        .route("/networks/{id}", get(get_network).delete(delete_network))
        .route(
            "/networks/{id}/policies",
            get(list_policies).post(create_policy),
        )
        .route("/networks/{id}/members", get(list_members).post(add_member))
        .route(
            "/networks/{id}/members/{workload_id}",
            delete(remove_member),
        )
}
