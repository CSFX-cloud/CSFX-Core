use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use axum::body::Body;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use chrono::Utc;
use entity::entities::{agent_api_keys, agents, logs, resource_groups, volumes, workloads};
use http_body_util::BodyExt;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common::{harness, Harness};

pub const FAILING_PREFIX: u128 = 0xdead_0000;
pub const MISSING_PREFIX: u128 = 0x404f_0000;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());
static GATEWAY_WORKLOADS: std::sync::Mutex<Vec<(Uuid, Uuid)>> = std::sync::Mutex::new(Vec::new());
static SCRIPTED: OnceLock<String> = OnceLock::new();

pub fn spawn_server(router: Router) -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
    });
    address
}

pub fn prefixed_id(prefix: u128) -> Uuid {
    Uuid::from_u128((prefix << 96) | (Uuid::new_v4().as_u128() >> 32))
}

fn scripted_status(method: &str, path: &str) -> StatusCode {
    let tail = path.strip_prefix("/workloads/").unwrap_or("");
    if method != "DELETE" {
        return StatusCode::OK;
    }
    if tail.starts_with("dead0000") {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    if tail.starts_with("404f0000") {
        return StatusCode::NOT_FOUND;
    }
    StatusCode::OK
}

fn scripted_reply(method: &str, path: &str, query: Option<&str>, body: Value) -> Value {
    let mut reply = json!({"method": method, "path": path, "query": query, "body": body});
    if path == "/internal/agent-proxy-ticket" {
        reply["payload"] = json!("scripted-payload");
        reply["signature"] = json!("scripted-signature");
    }
    if method == "POST" && path == "/workloads" {
        reply["workload_id"] = json!(scheduled_workload(&reply["body"]));
    }
    reply
}

fn scheduled_workload(body: &Value) -> Option<Uuid> {
    let group: Uuid = body["resource_group_id"].as_str()?.parse().ok()?;
    GATEWAY_WORKLOADS
        .lock()
        .unwrap()
        .iter()
        .find(|(registered, _)| *registered == group)
        .map(|(_, workload)| *workload)
}

pub fn expect_gateway_workload(group_id: Uuid, workload_id: Uuid) {
    GATEWAY_WORKLOADS
        .lock()
        .unwrap()
        .push((group_id, workload_id));
}

async fn scripted(request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
    let method = parts.method.as_str();
    let path = parts.uri.path();
    let status = scripted_status(method, path);
    let reply = scripted_reply(method, path, parts.uri.query(), body);
    (status, Json(reply)).into_response()
}

fn scripted_url() -> &'static str {
    SCRIPTED.get_or_init(|| {
        let address = spawn_server(Router::new().fallback(any(scripted)));
        format!("http://{address}")
    })
}

async fn echo_socket(mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.recv().await {
        if matches!(message, Message::Close(_)) || socket.send(message).await.is_err() {
            break;
        }
    }
}

async fn metrics_socket(mut socket: WebSocket) {
    let _ = socket.send(Message::Text("{\"cpu\":1}".into())).await;
    let _ = socket.send(Message::Close(None)).await;
}

async fn agent_logs(Path(id): Path<String>) -> Response {
    if id.starts_with("dead") {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    "first line\nsecond line\n".into_response()
}

async fn agent_power(Json(body): Json<Value>) -> StatusCode {
    match body["action"].as_str() {
        Some("poweroff") => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::NO_CONTENT,
    }
}

fn agent_router() -> Router {
    Router::new()
        .route("/logs/{id}", get(agent_logs))
        .route("/power", post(agent_power))
        .route("/disks", get(|| async { Json(json!([{"device": "sda"}])) }))
        .route(
            "/exec/{id}",
            get(|ws: WebSocketUpgrade| async { ws.on_upgrade(echo_socket) }),
        )
        .route(
            "/vnc/{id}",
            get(|ws: WebSocketUpgrade| async { ws.on_upgrade(echo_socket) }),
        )
        .route(
            "/metrics/stream",
            get(|ws: WebSocketUpgrade| async { ws.on_upgrade(metrics_socket) }),
        )
}

fn start_agent_server() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let address = spawn_server(agent_router());
        std::env::set_var("CSFX_AGENT_PORT", address.port().to_string());
    });
}

fn swap_env(names: &[&str], url: &str) -> Vec<(String, Option<String>)> {
    names
        .iter()
        .map(|name| {
            let previous = std::env::var(name).ok();
            std::env::set_var(name, url);
            (name.to_string(), previous)
        })
        .collect()
}

fn restore_env(saved: Vec<(String, Option<String>)>) {
    for (name, value) in saved {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

pub async fn plain_harness() -> Harness {
    let _guard = ENV_LOCK.lock().await;
    harness().await
}

pub async fn scripted_harness() -> Harness {
    start_agent_server();
    drop(harness().await);
    let _guard = ENV_LOCK.lock().await;
    let saved = swap_env(
        &["REGISTRY_SERVICE_URL", "SCHEDULER_SERVICE_URL"],
        scripted_url(),
    );
    let built = harness().await;
    restore_env(saved);
    built
}

pub async fn serve(harness: &Harness) -> SocketAddr {
    spawn_server(harness.app.clone())
}

pub fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

const CIDR_SPACE: u32 = 253 * 256;
const INSERT_ATTEMPTS: usize = 64;

static NEXT_CIDR: OnceLock<AtomicU32> = OnceLock::new();

pub fn random_cidr() -> String {
    let counter = NEXT_CIDR.get_or_init(|| {
        let seed = u32::from_le_bytes(Uuid::new_v4().as_bytes()[..4].try_into().unwrap());
        AtomicU32::new(seed % CIDR_SPACE)
    });
    let slot = counter.fetch_add(1, Ordering::Relaxed) % CIDR_SPACE;
    let octet = 1 + slot / 256;
    let octet = if octet >= 100 { octet + 1 } else { octet };
    format!("10.{}.{}.0/24", octet, slot % 256)
}

fn group_model(org_id: Uuid) -> resource_groups::ActiveModel {
    resource_groups::ActiveModel {
        id: Set(Uuid::new_v4()),
        organization_id: Set(org_id),
        name: Set(unique("group")),
        description: Set(None),
        internal_cidr: Set(random_cidr()),
        status: Set("active".to_string()),
        icon: Set("mdi:cube-outline".to_string()),
        color: Set("#6366f1".to_string()),
        pinned: Set(false),
        icon_image: Set(None),
        icon_image_mime: Set(None),
        preview_url: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    }
}

pub async fn insert_group(db: &DatabaseConnection, org_id: Uuid) -> resource_groups::Model {
    for _ in 0..INSERT_ATTEMPTS {
        match group_model(org_id).insert(db).await {
            Ok(group) => return group,
            Err(error) if is_unique_violation(&error) => continue,
            Err(error) => panic!("resource group insert failed: {error}"),
        }
    }
    panic!("no free resource group cidr found")
}

fn is_unique_violation(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

pub async fn insert_agent(db: &DatabaseConnection, role: &str, status: &str) -> agents::Model {
    let id = Uuid::new_v4();
    let octets = id.into_bytes();
    agents::ActiveModel {
        id: Set(id),
        name: Set(unique("agent")),
        hostname: Set(unique("node")),
        ip_address: Set(Some(format!("198.51.{}.{}", octets[0], octets[1]))),
        agent_version: Set("1.0.0".to_string()),
        os_type: Set("linux".to_string()),
        os_version: Set("6.1".to_string()),
        architecture: Set("x86_64".to_string()),
        status: Set(status.to_string()),
        last_heartbeat: Set(Some(Utc::now().naive_utc())),
        registered_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
        organization_id: Set(None),
        tags: Set(None),
        capabilities: Set(None),
        public_key_pem: Set(None),
        wg_public_key: Set(Some(unique("pubkey"))),
        wg_endpoint: Set(Some("192.0.2.10:51820".to_string())),
        wg_tunnel_ip: Set(Some("127.0.0.1".to_string())),
        kvm_capable: Set(false),
        ssh_host_key: Set(None),
        cordoned: Set(false),
        maintenance_until: Set(None),
        role: Set(role.to_string()),
        machine_id: Set(None),
        cpu_overcommit_ratio: Set(1.0),
        memory_overcommit_ratio: Set(1.0),
        disk_overcommit_ratio: Set(1.0),
    }
    .insert(db)
    .await
    .unwrap()
}

pub async fn update_agent(
    db: &DatabaseConnection,
    agent: agents::Model,
    change: impl FnOnce(&mut agents::ActiveModel),
) -> agents::Model {
    let mut active: agents::ActiveModel = agent.into();
    change(&mut active);
    active.update(db).await.unwrap()
}

pub async fn insert_api_key(db: &DatabaseConnection, agent_id: Uuid) -> String {
    let raw = unique("key");
    let digest = Sha256::digest(raw.as_bytes());
    let key_hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    agent_api_keys::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        key_hash: Set(key_hash),
        created_at: Set(Utc::now().naive_utc()),
        last_used_at: Set(None),
        is_active: Set(true),
    }
    .insert(db)
    .await
    .unwrap();
    raw
}

pub struct WorkloadSpec {
    pub id: Uuid,
    pub group_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub status: &'static str,
    pub guest_ip: Option<String>,
    pub ports: Option<Value>,
}

impl WorkloadSpec {
    pub fn new(group_id: Uuid, agent_id: Option<Uuid>, status: &'static str) -> Self {
        Self {
            id: Uuid::new_v4(),
            group_id: Some(group_id),
            agent_id,
            org_id: None,
            status,
            guest_ip: None,
            ports: None,
        }
    }
}

pub async fn insert_workload(db: &DatabaseConnection, spec: WorkloadSpec) -> workloads::Model {
    workloads::ActiveModel {
        id: Set(spec.id),
        name: Set(unique("workload")),
        image: Set("nginx:latest".to_string()),
        cpu_millicores: Set(100),
        memory_bytes: Set(1024),
        disk_bytes: Set(1024),
        cpu_limit_millicores: Set(None),
        memory_limit_bytes: Set(None),
        spread_key: Set(None),
        guest_ip: Set(spec.guest_ip),
        auto_resources: Set(false),
        cpu_usage_peak_percent: Set(None),
        memory_usage_peak_bytes: Set(None),
        env_vars: Set(None),
        ports: Set(spec.ports),
        volume_mounts: Set(None),
        status: Set(spec.status.to_string()),
        assigned_agent_id: Set(spec.agent_id),
        container_id: Set(None),
        created_by: Set(None),
        organization_id: Set(spec.org_id),
        resource_group_id: Set(spec.group_id),
        stack_id: Set(None),
        service_name: Set(Some(unique("svc"))),
        restart_policy: Set("always".to_string()),
        max_restarts: Set(None),
        restart_count: Set(0),
        runtime_class: Set("container".to_string()),
        desired_state: Set("running".to_string()),
        restart_requested: Set(false),
        cpu_usage_percent: Set(None),
        memory_usage_bytes: Set(None),
        network_rx_bytes: Set(None),
        network_tx_bytes: Set(None),
        volume_usage: Set(None),
        stats_updated_at: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap()
}

pub async fn raw(
    harness: &Harness,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> (StatusCode, HeaderMap, Bytes) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let mut request = builder.body(Body::from(body)).unwrap();
    request.extensions_mut().insert(axum::extract::ConnectInfo(
        "127.0.0.1:4000".parse::<SocketAddr>().unwrap(),
    ));
    let response = harness.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes)
}

pub async fn keyed(
    harness: &Harness,
    method: Method,
    uri: &str,
    key: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let payload = body.map(|value| value.to_string().into_bytes());
    let (status, _, bytes) = raw(
        harness,
        method,
        uri,
        &[("x-api-key", key), ("content-type", "application/json")],
        payload.unwrap_or_default(),
    )
    .await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

pub async fn insert_volume(
    db: &DatabaseConnection,
    agent_id: Option<Uuid>,
    workload_id: Option<Uuid>,
) -> volumes::Model {
    volumes::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(unique("vol")),
        size_gb: Set(1),
        pool: Set("pool".to_string()),
        image_name: Set("image".to_string()),
        status: Set("attached".to_string()),
        attached_to_agent: Set(agent_id),
        attached_to_workload: Set(workload_id),
        mapped_device: Set(None),
        organization_id: Set(None),
        resource_group_id: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap()
}

pub struct LogSpec<'a> {
    pub service: &'a str,
    pub level: &'a str,
    pub kind: &'a str,
    pub message: &'a str,
    pub action: Option<&'a str>,
}

pub async fn insert_log(db: &DatabaseConnection, spec: LogSpec<'_>) -> logs::Model {
    let now = (Utc::now() - chrono::Duration::minutes(5)).fixed_offset();
    logs::ActiveModel {
        id: Set(Uuid::new_v4()),
        service: Set(spec.service.to_string()),
        level: Set(spec.level.to_string()),
        classification: Set("system".to_string()),
        message: Set(spec.message.to_string()),
        agent_id: Set(None),
        workload_id: Set(None),
        organization_id: Set(None),
        created_at: Set(now),
        kind: Set(spec.kind.to_string()),
        actor_user_id: Set(None),
        action: Set(spec.action.map(str::to_string)),
        target_type: Set(spec.action.map(|_| "workload".to_string())),
        target_id: Set(None),
        fields: Set(json!({})),
        occurrences: Set(1),
        last_seen_at: Set(now),
    }
    .insert(db)
    .await
    .unwrap()
}
