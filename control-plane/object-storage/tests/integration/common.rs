use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::extract::{Path, Query, Request};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use object_storage::crypto::SecretBox;
use object_storage::garage::{GarageClient, S3Client};
use object_storage::server::{create_router, AppState};
use sea_orm::{Database, DatabaseConnection};
use serde_json::{json, Value};
use std::collections::HashMap;
use tower::ServiceExt;
use uuid::Uuid;

const ENCRYPTION_KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
static MOCK_URL: OnceLock<String> = OnceLock::new();

fn failure_marker(value: &str) -> Option<&'static str> {
    [
        "failquota",
        "failallow",
        "faildelete",
        "failcreate",
        "failkey",
    ]
    .into_iter()
    .find(|marker| value.starts_with(marker))
}

async fn create_bucket(Json(body): Json<Value>) -> Response {
    let alias = body["globalAlias"].as_str().unwrap_or_default();
    match failure_marker(alias) {
        Some("failcreate") => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Some(marker) => {
            Json(json!({"id": format!("{}-{}", marker, Uuid::new_v4().simple())})).into_response()
        }
        None => Json(json!({"id": format!("gb-{}", Uuid::new_v4().simple())})).into_response(),
    }
}

async fn update_bucket(Query(query): Query<HashMap<String, String>>) -> StatusCode {
    match query.get("id").map(String::as_str).and_then(failure_marker) {
        Some("failquota") => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::OK,
    }
}

async fn delete_bucket(Query(query): Query<HashMap<String, String>>) -> StatusCode {
    match query.get("id").map(String::as_str).and_then(failure_marker) {
        Some("faildelete") => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::OK,
    }
}

async fn create_key(Json(body): Json<Value>) -> Response {
    let name = body["name"].as_str().unwrap_or_default();
    if name.contains("failkey") {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    Json(json!({
        "accessKeyId": format!("GK{}", Uuid::new_v4().simple()),
        "secretAccessKey": "mock-secret"
    }))
    .into_response()
}

async fn allow_bucket_key(Json(body): Json<Value>) -> StatusCode {
    let bucket = body["bucketId"].as_str().unwrap_or_default();
    match failure_marker(bucket) {
        Some("failallow") => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::OK,
    }
}

async fn s3_list(Path(bucket): Path<String>) -> Response {
    if bucket.starts_with("s3fail") {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let xml = "<ListBucketResult><Contents><Key>a.txt</Key><Size>12</Size><LastModified>2026-01-01T00:00:00Z</LastModified></Contents><Contents><Key></Key></Contents><CommonPrefixes><Prefix>docs/</Prefix></CommonPrefixes></ListBucketResult>";
    xml.into_response()
}

async fn s3_delete(Path((bucket, _key)): Path<(String, String)>) -> StatusCode {
    if bucket.starts_with("s3fail") {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::NO_CONTENT
    }
}

fn mock_router() -> Router {
    Router::new()
        .route("/v2/CreateBucket", post(create_bucket))
        .route("/v2/UpdateBucket", post(update_bucket))
        .route("/v2/DeleteBucket", post(delete_bucket))
        .route("/v2/CreateKey", post(create_key))
        .route("/v2/DeleteKey", post(|| async { StatusCode::OK }))
        .route("/v2/AllowBucketKey", post(allow_bucket_key))
        .route("/{bucket}", get(s3_list))
        .route("/{bucket}/{*key}", any(s3_delete))
}

fn mock_url() -> &'static str {
    MOCK_URL.get_or_init(|| {
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std_listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", std_listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();
                axum::serve(listener, mock_router()).await.unwrap();
            });
        });
        std::env::set_var("OBJECT_STORAGE_ENCRYPTION_KEY", ENCRYPTION_KEY);
        url
    })
}

pub async fn database() -> DatabaseConnection {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = Database::connect(&url)
        .await
        .expect("database must be reachable");
    MIGRATED
        .get_or_init(|| async {
            Migrator::up(&db, None)
                .await
                .expect("migrations must apply");
        })
        .await;
    db
}

pub async fn router() -> Router {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let url = mock_url().to_string();
    let state = AppState::new(
        database().await,
        GarageClient::new(url.clone(), "token".to_string()),
        S3Client::new(url, "http://public.csfx.test".to_string()),
        Arc::new(SecretBox::from_env().unwrap()),
    );
    create_router(state)
}

pub async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let payload = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let request: Request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(payload)
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

pub async fn create_named_bucket(app: &Router, name: &str) -> (StatusCode, Value) {
    call(
        app,
        Method::POST,
        "/buckets",
        Some(json!({"name": name, "exposure": "private"})),
    )
    .await
}

pub async fn new_bucket(app: &Router) -> String {
    let (status, bucket) =
        create_named_bucket(app, &format!("bucket-{}", Uuid::new_v4().simple())).await;
    assert_eq!(status, StatusCode::CREATED);
    bucket["id"].as_str().unwrap().to_string()
}
