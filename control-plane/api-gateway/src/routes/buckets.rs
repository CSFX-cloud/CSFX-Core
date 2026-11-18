use axum::{
    body::Bytes,
    extract::{Path, Query, RawQuery, State},
    http::{HeaderMap, Method},
    response::Response,
    routing::{delete, get, patch, post},
    Router,
};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageBuckets, CanViewBuckets},
    routes::proxy::{proxy, with_query, ProxyError},
    service_client::Service,
    AppState,
};

async fn object_storage(
    state: &AppState,
    method: Method,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    proxy(state, Service::ObjectStorage, method, path, headers, body).await
}

pub async fn create_bucket(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    object_storage(&state, Method::POST, "/buckets", &headers, body).await
}

pub async fn list_buckets(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<Response, ProxyError> {
    let path = with_query("/buckets", query);
    object_storage(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn get_bucket(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{id}");
    object_storage(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn update_bucket(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{id}");
    object_storage(&state, Method::PATCH, &path, &headers, body).await
}

pub async fn delete_bucket(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{id}");
    object_storage(&state, Method::DELETE, &path, &headers, Bytes::new()).await
}

pub async fn list_keys(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{id}/keys");
    object_storage(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn create_key(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{id}/keys");
    object_storage(&state, Method::POST, &path, &headers, body).await
}

pub async fn rotate_key(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{bucket_id}/keys/{key_id}/rotate");
    object_storage(&state, Method::POST, &path, &headers, Bytes::new()).await
}

pub async fn delete_key(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{bucket_id}/keys/{key_id}");
    object_storage(&state, Method::DELETE, &path, &headers, Bytes::new()).await
}

pub async fn get_cluster_status(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    object_storage(&state, Method::GET, "/cluster", &headers, Bytes::new()).await
}

#[derive(Deserialize)]
pub struct ListObjectsQuery {
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    continuation_token: Option<String>,
}

pub async fn list_objects(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListObjectsQuery>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let mut path = format!(
        "/buckets/{id}/objects?prefix={}",
        utf8_percent_encode(&query.prefix, NON_ALPHANUMERIC)
    );
    if let Some(token) = &query.continuation_token {
        path.push_str(&format!(
            "&continuation_token={}",
            utf8_percent_encode(token, NON_ALPHANUMERIC)
        ));
    }
    object_storage(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn delete_object(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{bucket_id}/objects/{key}");
    object_storage(&state, Method::DELETE, &path, &headers, Bytes::new()).await
}

pub async fn presign_upload(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{id}/objects/presign-upload");
    object_storage(&state, Method::POST, &path, &headers, body).await
}

pub async fn presign_download(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/buckets/{bucket_id}/objects/presign-download/{key}");
    object_storage(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub fn buckets_routes() -> Router<AppState> {
    Router::new()
        .route("/buckets", post(create_bucket))
        .route("/buckets", get(list_buckets))
        .route("/buckets/{id}", get(get_bucket))
        .route("/buckets/{id}", patch(update_bucket))
        .route("/buckets/{id}", delete(delete_bucket))
        .route("/buckets/{id}/keys", get(list_keys))
        .route("/buckets/{id}/keys", post(create_key))
        .route(
            "/buckets/{bucket_id}/keys/{key_id}/rotate",
            post(rotate_key),
        )
        .route("/buckets/{bucket_id}/keys/{key_id}", delete(delete_key))
        .route("/buckets/{id}/objects", get(list_objects))
        .route("/buckets/{id}/objects/presign-upload", post(presign_upload))
        .route(
            "/buckets/{bucket_id}/objects/presign-download/{*key}",
            get(presign_download),
        )
        .route("/buckets/{bucket_id}/objects/{*key}", delete(delete_object))
        .route("/object-storage/cluster", get(get_cluster_status))
}
