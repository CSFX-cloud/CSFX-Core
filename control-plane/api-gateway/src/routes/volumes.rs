use axum::{
    body::Bytes,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, Method},
    response::Response,
    routing::{delete, get, post},
    Router,
};
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageVolumes, CanViewVolumes},
    routes::proxy::{proxy, with_query, ProxyError},
    service_client::Service,
    AppState,
};

async fn volume_manager(
    state: &AppState,
    method: Method,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    proxy(state, Service::VolumeManager, method, path, headers, body).await
}

pub async fn create_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    volume_manager(&state, Method::POST, "/volumes", &headers, body).await
}

pub async fn list_volumes(
    CanViewVolumes(_claims): CanViewVolumes,
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<Response, ProxyError> {
    let path = with_query("/volumes", query);
    volume_manager(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn get_volume(
    CanViewVolumes(_claims): CanViewVolumes,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/volumes/{id}");
    volume_manager(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn delete_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/volumes/{id}");
    volume_manager(&state, Method::DELETE, &path, &headers, Bytes::new()).await
}

pub async fn attach_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/volumes/{id}/attach");
    volume_manager(&state, Method::POST, &path, &headers, body).await
}

pub async fn resize_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/volumes/{id}/resize");
    volume_manager(&state, Method::POST, &path, &headers, body).await
}

pub async fn detach_volume(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/volumes/{id}/detach");
    volume_manager(&state, Method::POST, &path, &headers, Bytes::new()).await
}

pub async fn create_snapshot(
    CanManageVolumes(_claims): CanManageVolumes,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/volumes/{id}/snapshots");
    volume_manager(&state, Method::POST, &path, &headers, body).await
}

pub async fn list_snapshots(
    CanViewVolumes(_claims): CanViewVolumes,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/volumes/{id}/snapshots");
    volume_manager(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub fn volumes_routes() -> Router<AppState> {
    Router::new()
        .route("/volumes", post(create_volume))
        .route("/volumes", get(list_volumes))
        .route("/volumes/{id}", get(get_volume))
        .route("/volumes/{id}", delete(delete_volume))
        .route("/volumes/{id}/attach", post(attach_volume))
        .route("/volumes/{id}/resize", post(resize_volume))
        .route("/volumes/{id}/detach", post(detach_volume))
        .route("/volumes/{id}/snapshots", post(create_snapshot))
        .route("/volumes/{id}/snapshots", get(list_snapshots))
}
