use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::body::Body;
use axum::extract::Path;
use axum::http::{Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use chrono::{Duration, NaiveDateTime, Utc};
use entity::entities::{agent_metrics, agents, workloads};
use failover_controller::server::{create_router, AppState};
use failover_controller::services::eviction::EvictionService;
use failover_controller::services::failover::FailoverService;
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, Set};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
static MOCK_URL: OnceLock<String> = OnceLock::new();
static PLANS: OnceLock<Mutex<HashMap<Uuid, Plan>>> = OnceLock::new();
static CALLS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
pub static STALE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone)]
pub struct Plan {
    pub detach: u16,
    pub reschedule: u16,
    pub reschedule_body: Value,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            detach: 200,
            reschedule: 200,
            reschedule_body: json!([{"status": "scheduled"}]),
        }
    }
}

fn plans() -> &'static Mutex<HashMap<Uuid, Plan>> {
    PLANS.get_or_init(Mutex::default)
}

fn calls() -> &'static Mutex<Vec<String>> {
    CALLS.get_or_init(Mutex::default)
}

pub fn set_plan(agent_id: Uuid, plan: Plan) {
    plans().lock().unwrap().insert(agent_id, plan);
}

pub fn recorded_calls(agent_id: Uuid) -> Vec<String> {
    let needle = agent_id.to_string();
    calls()
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call.contains(&needle))
        .cloned()
        .collect()
}

fn plan_for(agent_id: Uuid, route: &str) -> Plan {
    calls()
        .lock()
        .unwrap()
        .push(format!("{} {}", route, agent_id));
    plans()
        .lock()
        .unwrap()
        .get(&agent_id)
        .cloned()
        .unwrap_or_default()
}

async fn detach_all(Path(agent_id): Path<Uuid>) -> Response {
    let plan = plan_for(agent_id, "detach-all");
    StatusCode::from_u16(plan.detach).unwrap().into_response()
}

async fn reschedule(Path(agent_id): Path<Uuid>) -> Response {
    let plan = plan_for(agent_id, "reschedule");
    let status = StatusCode::from_u16(plan.reschedule).unwrap();
    (status, Json(plan.reschedule_body)).into_response()
}

fn mock_router() -> Router {
    Router::new()
        .route("/internal/agents/{id}/detach-all", post(detach_all))
        .route("/internal/agents/{id}/reschedule", post(reschedule))
}

pub fn mock_url() -> &'static str {
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
        std::env::set_var("SCHEDULER_SERVICE_URL", &url);
        std::env::set_var("VOLUME_MANAGER_URL", &url);
        url
    })
}

pub async fn database() -> DatabaseConnection {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
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

pub async fn router() -> (Router, DatabaseConnection) {
    let db = database().await;
    (create_router(AppState { db: db.clone() }), db)
}

pub fn failover_service(db: DatabaseConnection) -> FailoverService {
    mock_url();
    FailoverService::new(db)
}

pub fn eviction_service(db: DatabaseConnection) -> EvictionService {
    mock_url();
    EvictionService::new(db)
}

pub async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, text) = call_text(app, method, uri, body).await;
    let value = serde_json::from_str(&text).unwrap_or(Value::Null);
    (status, value)
}

pub async fn call_text(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, String) {
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
    (status, String::from_utf8_lossy(&bytes).to_string())
}

pub fn ago(seconds: i64) -> NaiveDateTime {
    Utc::now().naive_utc() - Duration::seconds(seconds)
}

pub async fn insert_agent(
    db: &DatabaseConnection,
    status: &str,
    heartbeat_age_secs: Option<i64>,
    maintenance_until: Option<NaiveDateTime>,
) -> agents::Model {
    let id = Uuid::new_v4();
    agents::ActiveModel {
        id: Set(id),
        name: Set(format!("fo-agent-{}", id.simple())),
        hostname: Set(format!("fo-host-{}", id.simple())),
        ip_address: Set(None),
        agent_version: Set("0.0.1".to_string()),
        os_type: Set("linux".to_string()),
        os_version: Set("1".to_string()),
        architecture: Set("x86_64".to_string()),
        status: Set(status.to_string()),
        last_heartbeat: Set(heartbeat_age_secs.map(ago)),
        registered_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
        organization_id: Set(None),
        tags: Set(None),
        capabilities: Set(None),
        public_key_pem: Set(None),
        wg_public_key: Set(None),
        wg_endpoint: Set(None),
        wg_tunnel_ip: Set(None),
        kvm_capable: Set(false),
        ssh_host_key: Set(None),
        cordoned: Set(false),
        maintenance_until: Set(maintenance_until),
        role: Set("worker".to_string()),
        machine_id: Set(None),
        cpu_overcommit_ratio: Set(1.0),
        memory_overcommit_ratio: Set(1.0),
        disk_overcommit_ratio: Set(1.0),
    }
    .insert(db)
    .await
    .expect("agent must insert")
}

pub struct WorkloadSpec {
    pub status: &'static str,
    pub cpu_millicores: i32,
    pub cpu_limit_millicores: Option<i32>,
    pub memory_usage_bytes: i64,
    pub volume_mounts: Option<Value>,
}

impl Default for WorkloadSpec {
    fn default() -> Self {
        Self {
            status: "running",
            cpu_millicores: 500,
            cpu_limit_millicores: Some(1000),
            memory_usage_bytes: 100,
            volume_mounts: None,
        }
    }
}

pub async fn insert_workload(
    db: &DatabaseConnection,
    agent_id: Uuid,
    spec: WorkloadSpec,
) -> workloads::Model {
    let id = Uuid::new_v4();
    workloads::ActiveModel {
        id: Set(id),
        name: Set(format!("fo-workload-{}", id.simple())),
        image: Set("nginx:latest".to_string()),
        cpu_millicores: Set(spec.cpu_millicores),
        memory_bytes: Set(1024),
        disk_bytes: Set(0),
        cpu_limit_millicores: Set(spec.cpu_limit_millicores),
        memory_limit_bytes: Set(None),
        auto_resources: Set(false),
        volume_mounts: Set(spec.volume_mounts),
        status: Set(spec.status.to_string()),
        assigned_agent_id: Set(Some(agent_id)),
        restart_policy: Set("always".to_string()),
        restart_count: Set(0),
        runtime_class: Set("firecracker".to_string()),
        desired_state: Set("running".to_string()),
        restart_requested: Set(false),
        memory_usage_bytes: Set(Some(spec.memory_usage_bytes)),
        created_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("workload must insert")
}

pub async fn insert_metric(
    db: &DatabaseConnection,
    agent_id: Uuid,
    usage: (f32, f32, f32),
    age_secs: i64,
) {
    agent_metrics::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        timestamp: Set(ago(age_secs)),
        cpu_usage_percent: Set(Some(usage.0)),
        memory_usage_percent: Set(Some(usage.1)),
        disk_usage_percent: Set(Some(usage.2)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("metric must insert");
}
