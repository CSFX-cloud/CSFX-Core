use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use api_gateway::openbao::OpenBao;
use api_gateway::site_client::SiteBuilder;
use api_gateway::{routes, service_client, AppState};
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use http_body_util::BodyExt;
use jsonwebtoken::{encode, EncodingKey, Header};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

use crate::common::{harness, Harness};

const SITE_SECRET: &str = "site-builder-test-secret";

#[derive(Default)]
struct Version {
    value: String,
    deletion_time: String,
}

#[derive(Default)]
struct Store {
    secrets: BTreeMap<String, Vec<Version>>,
    custom: BTreeMap<String, Value>,
}

type Shared = Arc<Mutex<Store>>;

static BAO_URL: OnceLock<String> = OnceLock::new();
static BUILDER_URL: OnceLock<String> = OnceLock::new();
static IDP_URL: OnceLock<String> = OnceLock::new();
static CA_READY: OnceLock<()> = OnceLock::new();
static PEER_COUNTER: AtomicU32 = AtomicU32::new(1);
static BUILDER_CALLS: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());

fn serve(router: Router) -> String {
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
            axum::serve(listener, router).await.unwrap();
        });
    });
    url
}

fn reply(status: StatusCode, body: Option<Value>) -> Response {
    match body {
        Some(value) => (status, Json(value)).into_response(),
        None => status.into_response(),
    }
}

fn data_read(versions: &[Version], query: Option<&str>) -> (StatusCode, Option<Value>) {
    let wanted = query
        .and_then(|q| q.strip_prefix("version="))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(versions.len());
    match versions.get(wanted.wrapping_sub(1)) {
        Some(version) if version.deletion_time.is_empty() => (
            StatusCode::OK,
            Some(json!({"data": {
                "data": {"value": version.value},
                "metadata": {"version": wanted},
            }})),
        ),
        _ => (StatusCode::NOT_FOUND, Some(json!({"errors": []}))),
    }
}

fn data_route(
    store: &mut Store,
    method: &Method,
    key: &str,
    query: Option<&str>,
    body: &Value,
) -> (StatusCode, Option<Value>) {
    if key.contains("boom") {
        return (StatusCode::INTERNAL_SERVER_ERROR, None);
    }
    if key.contains("reject") {
        return (StatusCode::BAD_REQUEST, Some(json!({"errors": ["bad"]})));
    }
    let versions = store.secrets.entry(key.to_string()).or_default();
    match *method {
        Method::POST => {
            versions.push(Version {
                value: body["data"]["value"].as_str().unwrap_or_default().into(),
                deletion_time: String::new(),
            });
            (
                StatusCode::OK,
                Some(json!({"data": {"version": versions.len()}})),
            )
        }
        Method::DELETE => {
            if let Some(last) = versions.last_mut() {
                last.deletion_time = "2026-01-01T00:00:00Z".to_string();
            }
            (StatusCode::NO_CONTENT, None)
        }
        _ => data_read(versions, query),
    }
}

fn listing(store: &Store, prefix: &str) -> (StatusCode, Option<Value>) {
    let keys: Vec<&str> = store
        .secrets
        .keys()
        .filter_map(|key| key.strip_prefix(prefix))
        .collect();
    if keys.is_empty() {
        return (StatusCode::NOT_FOUND, Some(json!({"errors": []})));
    }
    (StatusCode::OK, Some(json!({"data": {"keys": keys}})))
}

fn metadata_document(store: &Store, key: &str) -> (StatusCode, Option<Value>) {
    let Some(versions) = store.secrets.get(key).filter(|v| !v.is_empty()) else {
        return (StatusCode::NOT_FOUND, Some(json!({"errors": []})));
    };
    let entries: serde_json::Map<String, Value> = versions
        .iter()
        .enumerate()
        .map(|(index, version)| {
            (
                (index + 1).to_string(),
                json!({
                    "created_time": "2026-01-01T00:00:00Z",
                    "deletion_time": version.deletion_time,
                    "destroyed": false,
                }),
            )
        })
        .collect();
    let custom = store.custom.get(key).cloned().unwrap_or_else(|| json!({}));
    (
        StatusCode::OK,
        Some(json!({"data": {
            "current_version": versions.len(),
            "updated_time": "2026-01-01T00:00:00Z",
            "custom_metadata": custom,
            "versions": entries,
        }})),
    )
}

fn metadata_route(
    store: &mut Store,
    method: &Method,
    key: &str,
    query: Option<&str>,
    body: &Value,
) -> (StatusCode, Option<Value>) {
    if query == Some("list=true") {
        return listing(store, key);
    }
    match *method {
        Method::POST => {
            store
                .custom
                .insert(key.to_string(), body["custom_metadata"].clone());
            (StatusCode::NO_CONTENT, None)
        }
        Method::DELETE => {
            store.secrets.remove(key);
            (StatusCode::NO_CONTENT, None)
        }
        _ => metadata_document(store, key),
    }
}

fn undelete(store: &mut Store, key: &str, body: &Value) -> (StatusCode, Option<Value>) {
    let index = body["versions"][0].as_u64().unwrap_or(0) as usize;
    match store
        .secrets
        .get_mut(key)
        .and_then(|versions| versions.get_mut(index.wrapping_sub(1)))
    {
        Some(version) => {
            version.deletion_time.clear();
            (StatusCode::NO_CONTENT, None)
        }
        None => (StatusCode::NOT_FOUND, Some(json!({"errors": []}))),
    }
}

fn bao_route(
    store: &mut Store,
    method: &Method,
    path: &str,
    query: Option<&str>,
    body: &Value,
) -> (StatusCode, Option<Value>) {
    if let Some(key) = path.strip_prefix("csfx-kv/data/") {
        return data_route(store, method, key, query, body);
    }
    if let Some(key) = path.strip_prefix("csfx-kv/metadata/") {
        return metadata_route(store, method, key, query, body);
    }
    if let Some(key) = path.strip_prefix("csfx-kv/undelete/") {
        return undelete(store, key, body);
    }
    match path {
        "auth/approle/login" => (
            StatusCode::OK,
            Some(json!({"auth": {"client_token": "mock-token", "lease_duration": 3600}})),
        ),
        "sys/health" => (StatusCode::OK, Some(json!({}))),
        _ if path.ends_with("/role-id") => (
            StatusCode::OK,
            Some(json!({"data": {"role_id": "mock-role-id"}})),
        ),
        _ if path.ends_with("/secret-id") => (
            StatusCode::OK,
            Some(json!({"wrap_info": {"token": "mock-wrap"}})),
        ),
        _ => (StatusCode::NO_CONTENT, None),
    }
}

async fn bao_handler(
    State(store): State<Shared>,
    method: Method,
    uri: Uri,
    bytes: Bytes,
) -> Response {
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let path = uri.path().trim_start_matches("/v1/");
    let mut guard = store.lock().unwrap();
    let (status, payload) = bao_route(&mut guard, &method, path, uri.query(), &body);
    reply(status, payload)
}

fn detection(repo_url: &str) -> Value {
    if repo_url.contains("undetectable") {
        return json!({"detection": null});
    }
    if repo_url.contains("container") {
        return json!({"detection": {
            "framework": "node", "mode": "container", "install_command": "npm ci",
            "build_command": null, "output_dir": null, "start_command": "npm start",
            "port": 3000,
        }});
    }
    json!({"detection": {
        "framework": "vite", "mode": "static", "install_command": "npm ci",
        "build_command": "npm run build", "output_dir": "dist", "start_command": null,
        "port": null,
    }})
}

async fn builder_handler(method: Method, uri: Uri, bytes: Bytes) -> Response {
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let path = uri.path().to_string();
    let rejected = body.to_string().contains("builder-reject");
    BUILDER_CALLS
        .lock()
        .unwrap()
        .push((path.clone(), body.clone()));
    if rejected {
        return reply(
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(json!({"error": "refused"})),
        );
    }
    if method == Method::POST && path == "/internal/detect" {
        let repo_url = body["repo_url"].as_str().unwrap_or_default();
        return reply(StatusCode::OK, Some(detection(repo_url)));
    }
    if path == "/internal/registries/images" {
        let prefix = body["prefix"].as_str().unwrap_or_default();
        let repository = format!("{prefix}app/web");
        let listing = json!({"repositories": [{"repository": repository, "tags": ["v1"]}]});
        return reply(StatusCode::OK, Some(listing));
    }
    reply(StatusCode::OK, Some(json!({})))
}

fn write_credentials() -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!("csfx-openbao-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&directory).unwrap();
    let pair = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let certificate = pair.cert.pem();
    let files = [
        ("openbao-ca", certificate.clone()),
        ("openbao-client-certificate", certificate),
        ("openbao-client-key", pair.signing_key.serialize_pem()),
        ("openbao-role-id", "role".to_string()),
        ("openbao-secret-id", "secret".to_string()),
    ];
    for (name, content) in files {
        std::fs::write(directory.join(name), content).unwrap();
    }
    directory
}

fn bao_url() -> &'static str {
    BAO_URL.get_or_init(|| {
        let store: Shared = Shared::default();
        let router = Router::new().fallback(bao_handler).with_state(store);
        let url = serve(router);
        std::env::set_var("CREDENTIALS_DIRECTORY", write_credentials());
        std::env::set_var("OPENBAO_ADDRS", &url);
        url
    })
}

fn builder_url() -> &'static str {
    BUILDER_URL.get_or_init(|| {
        let url = serve(Router::new().fallback(builder_handler));
        std::env::set_var("SITE_BUILDER_URL", &url);
        std::env::set_var("CSFX_SITE_SECRET", SITE_SECRET);
        url
    })
}

pub fn builder_calls() -> Vec<(String, Value)> {
    BUILDER_CALLS.lock().unwrap().clone()
}

pub fn full_state(harness: &Harness) -> AppState {
    bao_url();
    builder_url();
    install_ca();
    AppState {
        db_conn: harness.db.clone(),
        agent_db: harness.db.clone(),
        service_client: service_client::ServiceClient::new(),
        default_org_id: Some(harness.org_id),
        agent_stream_registry: routes::agent_stream::AgentStreamRegistry::new(),
        openbao: Some(OpenBao::from_environment().unwrap()),
        site_builder: SiteBuilder::from_environment(),
    }
}

pub async fn full_harness() -> Harness {
    let mut harness = harness().await;
    harness.app = routes::create_router().with_state(full_state(&harness));
    harness
}

pub async fn resource_group(harness: &Harness) -> Uuid {
    crate::support_a::insert_group(&harness.db, harness.org_id)
        .await
        .id
}

pub const IDP_CLIENT_SECRET: &str = "idp-client-secret";
pub const IDP_TENANT: &str = "tenant";

fn host_url(headers: &HeaderMap) -> String {
    let host = headers["host"].to_str().unwrap();
    format!("http://{host}")
}

async fn idp_discovery(headers: HeaderMap, uri: Uri) -> Response {
    let base = host_url(&headers);
    let tenant = uri
        .path()
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default();
    let issuer = if tenant == "mismatch" {
        format!("{base}/other")
    } else {
        format!("{base}/{tenant}")
    };
    reply(
        StatusCode::OK,
        Some(json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{base}/authorize"),
            "token_endpoint": format!("{base}/token"),
            "jwks_uri": format!("{base}/jwks"),
        })),
    )
}

fn identity_claims(base: &str, audience: &str, code: &str) -> Value {
    let fields: Vec<&str> = code.split('~').collect();
    let field = |index: usize| fields.get(index).copied().unwrap_or_default();
    let groups: Vec<&str> = field(4)
        .split(',')
        .filter(|group| !group.is_empty())
        .collect();
    json!({
        "iss": format!("{base}/{IDP_TENANT}"),
        "aud": audience,
        "exp": chrono::Utc::now().timestamp() + 300,
        "sub": field(1),
        "nonce": field(0),
        "preferred_username": field(2),
        "email": field(3),
        "email_verified": true,
        "groups": groups,
    })
}

async fn idp_token(
    headers: HeaderMap,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> Response {
    let code = form.get("code").cloned().unwrap_or_default();
    if code.starts_with("deny") {
        return reply(
            StatusCode::BAD_REQUEST,
            Some(json!({"error": "invalid_grant"})),
        );
    }
    let claims = identity_claims(&host_url(&headers), &form["client_id"], &code);
    let key = EncodingKey::from_secret(form["client_secret"].as_bytes());
    let token = encode(&Header::default(), &claims, &key).unwrap();
    reply(StatusCode::OK, Some(json!({"id_token": token})))
}

async fn close_connection(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("close"));
    response
}

pub fn idp_issuer() -> String {
    let base = IDP_URL.get_or_init(|| {
        let router = Router::new()
            .route("/token", axum::routing::post(idp_token))
            .fallback(idp_discovery)
            .layer(axum::middleware::map_response(close_connection));
        serve(router)
    });
    format!("{base}/{IDP_TENANT}")
}

pub fn idp_mismatch_issuer() -> String {
    idp_issuer().replace(IDP_TENANT, "mismatch")
}

pub fn idp_code(nonce: &str, subject: &str, username: &str, groups: &str) -> String {
    format!("{nonce}~{subject}~{username}~{username}@idp.test~{groups}")
}

pub const SETUP_CODE: &str = "ABCD-EFGH-1234";

fn install_ca() {
    CA_READY.get_or_init(|| {
        let directory =
            std::env::temp_dir().join(format!("csfx-setup-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("setup-code"), SETUP_CODE).unwrap();
        std::env::set_var("CSFX_SETUP_CODE_FILE", directory.join("setup-code"));
        std::env::set_var("CSFX_JOIN_REQUEST_FILE", directory.join("join-request"));
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::new(Vec::new()).unwrap();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let certificate = params.self_signed(&key).unwrap();
        std::env::set_var("CSFX_CA_CERT_PEM", certificate.pem());
        std::env::set_var("CSFX_CA_KEY_PEM", key.serialize_pem());
    });
}

pub fn agent_key_hash(key: &str) -> String {
    Sha256::digest(key.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub struct AgentSpec {
    pub role: &'static str,
    pub tunnel_ip: Option<&'static str>,
    pub age_minutes: i64,
    pub cordoned: bool,
}

pub async fn agent_with_key(harness: &Harness, spec: AgentSpec) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now().naive_utc();
    entity::entities::agents::ActiveModel {
        id: Set(id),
        name: Set(format!("node-{}", id.simple())),
        hostname: Set(format!("host-{}", id.simple())),
        ip_address: Set(None),
        agent_version: Set("1.0.0".to_string()),
        os_type: Set("linux".to_string()),
        os_version: Set("1".to_string()),
        architecture: Set("x86_64".to_string()),
        status: Set("online".to_string()),
        last_heartbeat: Set(Some(now)),
        registered_at: Set(now - chrono::Duration::minutes(spec.age_minutes)),
        updated_at: Set(None),
        organization_id: Set(Some(harness.org_id)),
        tags: Set(None),
        capabilities: Set(None),
        public_key_pem: Set(None),
        wg_public_key: Set(None),
        wg_endpoint: Set(None),
        wg_tunnel_ip: Set(spec.tunnel_ip.map(str::to_string)),
        kvm_capable: Set(false),
        ssh_host_key: Set(None),
        cordoned: Set(spec.cordoned),
        maintenance_until: Set(None),
        role: Set(spec.role.to_string()),
        machine_id: Set(None),
        cpu_overcommit_ratio: Set(1.0),
        memory_overcommit_ratio: Set(1.0),
        disk_overcommit_ratio: Set(1.0),
    }
    .insert(&harness.db)
    .await
    .unwrap();
    let key = format!("key-{}", id.simple());
    entity::entities::agent_api_keys::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(id),
        key_hash: Set(agent_key_hash(&key)),
        created_at: Set(now),
        last_used_at: Set(None),
        is_active: Set(true),
    }
    .insert(&harness.db)
    .await
    .unwrap();
    (id, key)
}

pub struct Raw {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}

pub async fn raw(
    harness: &Harness,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> Raw {
    let octet = PEER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let peer: SocketAddr = format!("127.{}.{}.1:4000", octet / 250, octet % 250)
        .parse()
        .unwrap();
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let payload = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let mut request = builder.body(payload).unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    let response = harness.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Raw {
        status,
        headers,
        body,
    }
}
