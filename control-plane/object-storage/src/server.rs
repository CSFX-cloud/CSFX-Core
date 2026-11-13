use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::{
    crypto::SecretBox,
    garage::{GarageClient, S3Client},
    handlers::{buckets, cluster, keys, objects},
};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub garage: GarageClient,
    pub s3: S3Client,
    pub secret_box: Arc<SecretBox>,
}

impl AppState {
    pub fn new(
        db: DatabaseConnection,
        garage: GarageClient,
        s3: S3Client,
        secret_box: Arc<SecretBox>,
    ) -> Self {
        Self {
            db,
            garage,
            s3,
            secret_box,
        }
    }
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Object Storage OK")
}

pub fn create_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/health", get(health_check))
        .route(
            "/buckets",
            get(buckets::list_buckets).post(buckets::create_bucket),
        )
        .route(
            "/buckets/{id}",
            get(buckets::get_bucket)
                .patch(buckets::update_bucket)
                .delete(buckets::delete_bucket),
        )
        .route(
            "/buckets/{id}/keys",
            get(keys::list_keys).post(keys::create_key),
        )
        .route(
            "/buckets/{id}/keys/{key_id}/rotate",
            axum::routing::post(keys::rotate_key),
        )
        .route(
            "/buckets/{id}/keys/{key_id}",
            axum::routing::delete(keys::delete_key),
        )
        .route("/buckets/{id}/objects", get(objects::list_objects))
        .route(
            "/buckets/{id}/objects/presign-upload",
            axum::routing::post(objects::presign_upload),
        )
        .route(
            "/buckets/{bucket_id}/objects/presign-download/{*key}",
            get(objects::presign_download),
        )
        .route(
            "/buckets/{bucket_id}/objects/{*key}",
            axum::routing::delete(objects::delete_object),
        )
        .route("/cluster", get(cluster::get_cluster_status))
        .with_state(state);
    shared::observability::instrument(router)
}
