use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use scheduler::models::workload::CreateWorkloadRequest;
use scheduler::server::{create_router, AppState};
use scheduler::services::scheduler::SchedulerService;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, Database, DatabaseConnection};
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

pub struct Placing {
    pub service: SchedulerService,
    pub db: DatabaseConnection,
    pub etcd: Arc<Mutex<etcd_client::Client>>,
}

pub async fn placing() -> Placing {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let db = migrated_database().await;
    let endpoints =
        std::env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| "http://127.0.0.1:2379".to_string());
    let etcd = etcd_client::Client::connect(endpoints.split(',').collect::<Vec<_>>(), None)
        .await
        .expect("etcd must be reachable");
    let etcd = Arc::new(Mutex::new(etcd));
    let service = SchedulerService::new(db.clone(), etcd.clone());
    insert_agent(&db).await;
    Placing { service, db, etcd }
}

pub async fn insert_agent(db: &DatabaseConnection) -> Uuid {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now().naive_utc();
    entity::entities::agents::ActiveModel {
        id: Set(id),
        name: Set(unique_name("agent")),
        hostname: Set(unique_name("host")),
        ip_address: Set(None),
        agent_version: Set("test".to_string()),
        os_type: Set("linux".to_string()),
        os_version: Set("test".to_string()),
        architecture: Set("x86_64".to_string()),
        status: Set("Online".to_string()),
        last_heartbeat: Set(Some(now)),
        registered_at: Set(now),
        updated_at: Set(None),
        organization_id: Set(None),
        tags: Set(None),
        capabilities: Set(None),
        public_key_pem: Set(None),
        wg_public_key: Set(None),
        wg_endpoint: Set(None),
        wg_tunnel_ip: Set(None),
        kvm_capable: Set(true),
        cordoned: Set(false),
        maintenance_until: Set(None),
        role: Set("worker".to_string()),
        machine_id: Set(None),
        cpu_overcommit_ratio: Set(1.0),
        memory_overcommit_ratio: Set(1.0),
        disk_overcommit_ratio: Set(1.0),
    }
    .insert(db)
    .await
    .expect("agent must be inserted");
    entity::entities::agent_metrics::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(id),
        timestamp: Set(now),
        cpu_cores: Set(Some(1000)),
        memory_total_bytes: Set(Some(1 << 40)),
        disk_total_bytes: Set(Some(1 << 46)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("agent metrics must be inserted");
    id
}

pub async fn insert_resource_group(db: &DatabaseConnection) -> Uuid {
    let now = chrono::Utc::now().naive_utc();
    let organization_id = Uuid::new_v4();
    entity::entities::organization::ActiveModel {
        id: Set(organization_id),
        name: Set(unique_name("org")),
        description: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("organization must be inserted");
    let bytes = Uuid::new_v4().into_bytes();
    let id = Uuid::new_v4();
    entity::entities::resource_groups::ActiveModel {
        id: Set(id),
        organization_id: Set(organization_id),
        name: Set(unique_name("rg")),
        description: Set(None),
        internal_cidr: Set(format!("10.{}.{}.0/24", bytes[0], bytes[1])),
        status: Set("active".to_string()),
        icon: Set("box".to_string()),
        color: Set("blue".to_string()),
        pinned: Set(false),
        icon_image: Set(None),
        icon_image_mime: Set(None),
        preview_url: Set(None),
        created_at: Set(now),
        updated_at: Set(None),
    }
    .insert(db)
    .await
    .expect("resource group must be inserted");
    id
}

pub fn small_request(name: &str) -> CreateWorkloadRequest {
    serde_json::from_value(json!({
        "name": name,
        "image": "nginx:latest",
        "cpu_millicores": 10,
        "memory_bytes": 1048576,
        "disk_bytes": 1048576
    }))
    .expect("request must deserialize")
}
