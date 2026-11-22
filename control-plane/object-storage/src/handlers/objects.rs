use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Json},
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    models::{ListObjectsQuery, PresignQuery, PresignUploadRequest},
    server::AppState,
    services::object as service,
};

pub async fn list_objects(
    State(state): State<AppState>,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<ListObjectsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::list_objects(
        &state.db,
        &state.s3,
        &state.secret_box,
        bucket_id,
        &query.prefix,
        query.continuation_token.as_deref(),
    )
    .await
    {
        Ok(Some(result)) => Ok(Json(json!(result))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to list objects");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn delete_object(
    State(state): State<AppState>,
    Path((bucket_id, key)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::delete_object(&state.db, &state.s3, &state.secret_box, bucket_id, &key).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete object");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn presign_upload(
    State(state): State<AppState>,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<PresignQuery>,
    Json(req): Json<PresignUploadRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::presign_upload(
        &state.db,
        &state.s3,
        &state.secret_box,
        bucket_id,
        &req.key,
        &query.origin,
    )
    .await
    {
        Ok(Some(result)) => Ok(Json(json!(result))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to presign upload");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn presign_download(
    State(state): State<AppState>,
    Path((bucket_id, key)): Path<(Uuid, String)>,
    Query(query): Query<PresignQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::presign_download(
        &state.db,
        &state.s3,
        &state.secret_box,
        bucket_id,
        &key,
        &query.origin,
    )
    .await
    {
        Ok(Some(result)) => Ok(Json(json!(result))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to presign download");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn export_bucket(
    State(state): State<AppState>,
    Path(bucket_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match service::export_bucket(&state.db, &state.s3, &state.secret_box, bucket_id).await {
        Ok(Some(receiver)) => {
            let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
                receiver.recv().await.map(|chunk| (chunk, receiver))
            });
            Ok((
                [(CONTENT_TYPE, "application/x-tar")],
                Body::from_stream(stream),
            ))
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to start bucket export");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}

pub async fn import_bucket(
    State(state): State<AppState>,
    Path(bucket_id): Path<Uuid>,
    body: Body,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let stream = body.into_data_stream();
    match service::import_bucket(&state.db, &state.s3, &state.secret_box, bucket_id, stream).await {
        Ok(Some(objects)) => Ok(Json(json!({ "objects": objects }))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Bucket not found"})),
        )),
        Err(e) => {
            tracing::error!(error = %e, "failed to import bucket archive");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": e.to_string()})),
            ))
        }
    }
}
