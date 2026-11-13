use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;
use shared::pagination::{cursor_headers, split_page, PageParams};
use uuid::Uuid;

use crate::{
    models::{CreateBucketRequest, UpdateBucketRequest},
    server::AppState,
    services::bucket as service,
};

#[derive(Debug, Deserialize)]
pub struct ListBucketsQuery {
    pub resource_group_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
}

pub async fn create_bucket(
    State(state): State<AppState>,
    Json(req): Json<CreateBucketRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::create_bucket(&state.db, &state.garage, &state.secret_box, req).await {
        Ok(bucket) => Ok((StatusCode::CREATED, Json(json!(bucket)))),
        Err(e) => {
            tracing::error!(error = %e, "failed to create bucket");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn list_buckets(
    State(state): State<AppState>,
    Query(query): Query<ListBucketsQuery>,
    Query(page): Query<PageParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let after = page.cursor_id().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
    })?;
    let listed = service::list_buckets(
        &state.db,
        query.resource_group_id,
        query.organization_id,
        after,
        &page,
    )
    .await;
    match listed {
        Ok(buckets) => {
            let (buckets, next) = split_page(buckets, &page, |b| b.id.to_string());
            Ok((cursor_headers(next), Json(json!(buckets))))
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list buckets");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn get_bucket(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::get_bucket(&state.db, id).await {
        Ok(Some(bucket)) => Ok(Json(json!(bucket))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to get bucket");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn update_bucket(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateBucketRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::update_bucket(&state.db, &state.garage, id, req).await {
        Ok(Some(bucket)) => Ok(Json(json!(bucket))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to update bucket");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn delete_bucket(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::delete_bucket(&state.db, &state.garage, id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete bucket");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}
