use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use sdn_controller::server::{create_router, AppState};
use sdn_controller::services::ipam::IpamService;
use sea_orm::{Database, DatabaseConnection};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn migrated_database() -> DatabaseConnection {
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
    let endpoints = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS must be set");
    let etcd = etcd_client::Client::connect([endpoints.as_str()], None)
        .await
        .expect("etcd must be reachable");
    let state = AppState::new(migrated_database().await, IpamService::new(etcd));
    create_router(state)
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

pub async fn create_network(app: &Router, cidr: &str) -> String {
    let (status, network) = call(
        app,
        Method::POST,
        "/networks",
        Some(json!({
            "name": format!("net-{}", Uuid::new_v4().simple()),
            "cidr": cidr,
            "overlay_type": "vxlan",
            "resource_group_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    network["id"].as_str().unwrap().to_string()
}
