use axum::{
    body::Body,
    extract::{Path, Query, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageBuckets, CanViewBuckets},
    routes::paged_proxy::{proxy_list, with_query},
    service_client::ListTarget,
    AppState,
};

async fn proxy_to_object_storage(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    headers: Option<Vec<(String, String)>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state
        .service_client
        .forward_to_object_storage(method, path, body, headers)
        .await
    {
        Ok((status, Some(body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to object-storage: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(
                    json!({ "error": "Object Storage service unavailable", "details": e.to_string() }),
                ),
            ))
        }
    }
}

fn header_vec(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect()
}

pub async fn create_bucket(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::POST,
        "/buckets",
        body_json,
        Some(header_map),
    )
    .await
}

pub async fn list_buckets(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let path = with_query("/buckets", query);
    proxy_list(&state, ListTarget::ObjectStorage, &path, &headers).await
}

pub async fn get_bucket(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::GET,
        &format!("/buckets/{}", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn update_bucket(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::PATCH,
        &format!("/buckets/{}", id),
        body_json,
        Some(header_map),
    )
    .await
}

pub async fn delete_bucket(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::DELETE,
        &format!("/buckets/{}", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn list_keys(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::GET,
        &format!("/buckets/{}/keys", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn create_key(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::POST,
        &format!("/buckets/{}/keys", id),
        body_json,
        Some(header_map),
    )
    .await
}

pub async fn rotate_key(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::POST,
        &format!("/buckets/{}/keys/{}/rotate", bucket_id, key_id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn delete_key(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::DELETE,
        &format!("/buckets/{}/keys/{}", bucket_id, key_id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn get_cluster_status(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::GET,
        "/cluster",
        None,
        Some(header_map),
    )
    .await
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
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    let mut path = format!(
        "/buckets/{}/objects?prefix={}",
        id,
        percent_encoding::utf8_percent_encode(&query.prefix, percent_encoding::NON_ALPHANUMERIC)
    );
    if let Some(token) = &query.continuation_token {
        path.push_str(&format!(
            "&continuation_token={}",
            percent_encoding::utf8_percent_encode(token, percent_encoding::NON_ALPHANUMERIC)
        ));
    }
    proxy_to_object_storage(&state, reqwest::Method::GET, &path, None, Some(header_map)).await
}

pub async fn delete_object(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::DELETE,
        &format!("/buckets/{}/objects/{}", bucket_id, key),
        None,
        Some(header_map),
    )
    .await
}

pub async fn presign_upload(
    CanManageBuckets(_claims): CanManageBuckets,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::POST,
        &format!("/buckets/{}/objects/presign-upload", id),
        body_json,
        Some(header_map),
    )
    .await
}

pub async fn presign_download(
    CanViewBuckets(_claims): CanViewBuckets,
    State(state): State<AppState>,
    Path((bucket_id, key)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_object_storage(
        &state,
        reqwest::Method::GET,
        &format!("/buckets/{}/objects/presign-download/{}", bucket_id, key),
        None,
        Some(header_map),
    )
    .await
}

pub fn buckets_routes() -> Router<AppState> {
    Router::new()
        .route("/buckets", post(create_bucket))
        .route("/buckets", get(list_buckets))
        .route("/buckets/{id}", get(get_bucket))
        .route("/buckets/{id}", axum::routing::patch(update_bucket))
        .route("/buckets/{id}", axum::routing::delete(delete_bucket))
        .route("/buckets/{id}/keys", get(list_keys))
        .route("/buckets/{id}/keys", post(create_key))
        .route(
            "/buckets/{bucket_id}/keys/{key_id}/rotate",
            post(rotate_key),
        )
        .route(
            "/buckets/{bucket_id}/keys/{key_id}",
            axum::routing::delete(delete_key),
        )
        .route("/buckets/{id}/objects", get(list_objects))
        .route("/buckets/{id}/objects/presign-upload", post(presign_upload))
        .route(
            "/buckets/{bucket_id}/objects/presign-download/{*key}",
            get(presign_download),
        )
        .route(
            "/buckets/{bucket_id}/objects/{*key}",
            axum::routing::delete(delete_object),
        )
        .route("/object-storage/cluster", get(get_cluster_status))
}
