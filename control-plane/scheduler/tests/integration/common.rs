use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use scheduler::server::{create_router, AppState};
use scheduler::services::scheduler::SchedulerService;
use sea_orm::{Database, DatabaseConnection};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

pub async fn migrated_database() -> DatabaseConnection {
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
    let db = migrated_database().await;
    let etcd = etcd_client::Client::connect(["http://127.0.0.1:1"], None)
        .await
        .expect("etcd client must be constructed");
    let etcd = Arc::new(Mutex::new(etcd));
    let scheduler = Arc::new(SchedulerService::new(db.clone(), etcd.clone()));
    create_router(AppState {
        db,
        etcd,
        scheduler,
    })
}

pub fn unplaceable_request(name: &str) -> Value {
    json!({
        "name": name,
        "image": "nginx:latest",
        "cpu_millicores": 1,
        "memory_bytes": i64::MAX / 4,
        "disk_bytes": 1
    })
}

pub fn unique_name(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4().simple())
}

pub async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let payload = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let request = Request::builder()
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

pub async fn create_pending_workload(app: &Router) -> String {
    let (status, created) = call(
        app,
        Method::POST,
        "/workloads",
        Some(unplaceable_request(&unique_name("wl"))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    created["workload_id"].as_str().unwrap().to_string()
}

pub async fn find_workload(app: &Router, id: &str) -> Value {
    let (_, listed) = call(app, Method::GET, "/workloads", None).await;
    listed
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .cloned()
        .expect("workload must be listed")
}
