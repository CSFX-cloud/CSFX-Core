use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use chrono::Utc;
use entity::{site_deployments, site_domains, sites};
use flate2::{write::GzEncoder, Compression};
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use serde_json::{json, Value};
use site_builder::builds::Builds;
use site_builder::config::Config;
use site_builder::registry::client::RegistryClient;
use site_builder::registry::keys::{self, RegistryKeys};
use site_builder::scheduler::SchedulerClient;
use site_builder::server::control_router;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tower::ServiceExt;
use uuid::Uuid;

pub const SECRET: &str = "integration-secret";

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
static KEY_PEMS: OnceLock<(String, String)> = OnceLock::new();

fn relaxed_url(url: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}options=-c%20session_replication_role%3Dreplica")
}

pub fn install_crypto_provider() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
}

pub async fn migrated_database() -> DatabaseConnection {
    install_crypto_provider();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MIGRATED
        .get_or_init(|| async {
            let db = Database::connect(&url)
                .await
                .expect("database must be reachable");
            Migrator::up(&db, None)
                .await
                .expect("migrations must apply");
        })
        .await;
    Database::connect(&relaxed_url(&url))
        .await
        .expect("database must be reachable")
}

pub async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("127.0.0.1:{}", address.port())
}

#[derive(Clone, Default)]
pub struct SchedulerState {
    pub created: Arc<Mutex<Vec<Value>>>,
    pub deleted: Arc<Mutex<Vec<Uuid>>>,
    pub reject: Arc<AtomicBool>,
    ids: Arc<Mutex<Vec<Uuid>>>,
}

async fn create_workload(State(state): State<SchedulerState>, Json(body): Json<Value>) -> Response {
    if state.reject.load(Ordering::SeqCst) {
        let rejection = json!({ "error": "no capacity" });
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(rejection)).into_response();
    }
    let id = Uuid::new_v4();
    state.created.lock().unwrap().push(body);
    state.ids.lock().unwrap().push(id);
    (StatusCode::CREATED, Json(json!({ "workload_id": id }))).into_response()
}

async fn delete_workload(State(state): State<SchedulerState>, Path(id): Path<Uuid>) -> StatusCode {
    state.deleted.lock().unwrap().push(id);
    StatusCode::NO_CONTENT
}

async fn list_workloads(State(state): State<SchedulerState>) -> Json<Value> {
    let ids = state.ids.lock().unwrap().clone();
    Json(json!(ids
        .iter()
        .map(|id| json!({ "id": id, "status": "running" }))
        .collect::<Vec<_>>()))
}

pub async fn mock_scheduler() -> (String, SchedulerState) {
    let state = SchedulerState::default();
    let router = Router::new()
        .route("/workloads", get(list_workloads).post(create_workload))
        .route("/workloads/{id}", axum::routing::delete(delete_workload))
        .with_state(state.clone());
    (serve(router).await, state)
}

#[derive(Clone, Default)]
pub struct RegistryState {
    pub repositories: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

fn registry_response(state: &RegistryState, method: &Method, path: &str) -> Response {
    let segments: Vec<&str> = path.trim_start_matches("/v2/").split('/').collect();
    let mut repositories = state.repositories.lock().unwrap();
    if path == "/v2/_catalog" {
        let names: Vec<&String> = repositories.keys().collect();
        return Json(json!({ "repositories": names })).into_response();
    }
    if let Some(position) = segments.iter().rposition(|part| *part == "manifests") {
        let repository = segments[..position].join("/");
        let reference = segments[position + 1..].join("/");
        return manifest_response(&mut repositories, method, &repository, &reference);
    }
    let repository = segments[..segments.len() - 2].join("/");
    match repositories.get(&repository) {
        Some(tags) => Json(json!({ "tags": tags })).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn manifest_response(
    repositories: &mut HashMap<String, Vec<String>>,
    method: &Method,
    repository: &str,
    reference: &str,
) -> Response {
    let digest = format!("sha256:{}", reference.trim_start_matches("sha256:"));
    let known = repositories
        .get(repository)
        .is_some_and(|tags| tags.iter().any(|tag| digest.ends_with(tag.as_str())));
    if !known {
        return StatusCode::NOT_FOUND.into_response();
    }
    if method == Method::DELETE {
        repositories
            .get_mut(repository)
            .unwrap()
            .retain(|tag| !digest.ends_with(tag.as_str()));
        return StatusCode::ACCEPTED.into_response();
    }
    let manifest = json!({ "config": { "size": 10 }, "layers": [{ "size": 90 }] });
    ([("Docker-Content-Digest", digest)], Json(manifest)).into_response()
}

pub struct MockRegistry {
    pub address: String,
    pub state: RegistryState,
}

pub async fn mock_registry(repositories: &[(&str, &[&str])]) -> MockRegistry {
    let state = RegistryState::default();
    for (name, tags) in repositories {
        let tags = tags.iter().map(|tag| tag.to_string()).collect();
        state
            .repositories
            .lock()
            .unwrap()
            .insert(name.to_string(), tags);
    }
    let shared = state.clone();
    let handler = move |method: Method, uri: Uri| {
        let state = shared.clone();
        async move { registry_response(&state, &method, uri.path()) }
    };
    let address = serve(Router::new().fallback(any(handler))).await;
    MockRegistry { address, state }
}

pub fn registry_keys() -> Arc<RegistryKeys> {
    let (key, certificate) = KEY_PEMS.get_or_init(|| keys::generate().unwrap());
    Arc::new(RegistryKeys {
        encoding: jsonwebtoken::EncodingKey::from_rsa_pem(key.as_bytes()).unwrap(),
        key_id: "integration".to_string(),
        certificate_pem: certificate.clone(),
    })
}

pub fn registry_client(address: &str) -> RegistryClient {
    install_crypto_provider();
    RegistryClient::new(address, true, registry_keys()).unwrap()
}

pub fn config(scheduler_url: &str, registry: Option<&str>, preview: Option<&str>) -> Config {
    Config {
        port: 1,
        edge_port: 2,
        callback_url: "http://builder.invalid:8007".to_string(),
        secret: SECRET.to_string(),
        builder_image: "builder:test".to_string(),
        registry: registry.map(str::to_string),
        registry_insecure: true,
        registry_mirror: None,
        keep_deployments: 2,
        scheduler_url: scheduler_url.to_string(),
        preview_domain: preview.map(str::to_string),
        build_timeout: Duration::from_secs(900),
    }
}

pub struct Harness {
    pub app: Router,
    pub builds: Arc<Builds>,
    pub scheduler: SchedulerState,
    pub db: DatabaseConnection,
}

pub async fn harness(registry: Option<&MockRegistry>, preview: Option<&str>) -> Harness {
    let db = migrated_database().await;
    let (url, scheduler) = mock_scheduler().await;
    let address = registry.map(|mock| mock.address.as_str());
    let client = address.map(registry_client);
    let url = format!("http://{url}");
    install_crypto_provider();
    let builds = Arc::new(Builds::new(
        db.clone(),
        Arc::new(config(&url, address, preview)),
        SchedulerClient::new(&url).unwrap(),
        client,
    ));
    Harness {
        app: control_router(builds.clone()),
        builds,
        scheduler,
        db,
    }
}

pub async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

pub async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let authorization = bearer.map(|token| format!("Bearer {token}"));
    let mut headers = Vec::new();
    if body.is_some() {
        headers.push(("content-type", "application/json"));
    }
    if let Some(value) = authorization.as_deref() {
        headers.push(("authorization", value));
    }
    let payload = body.map(|value| value.to_string().into_bytes());
    let (status, bytes) = request(app, method, uri, &headers, payload.unwrap_or_default()).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

pub fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *data).unwrap();
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    std::io::Write::write_all(&mut encoder, &builder.into_inner().unwrap()).unwrap();
    encoder.finish().unwrap()
}

pub async fn relaxed(db: &DatabaseConnection) -> DatabaseTransaction {
    let transaction = db.begin().await.unwrap();
    transaction
        .execute_unprepared("SET LOCAL session_replication_role = replica")
        .await
        .unwrap();
    transaction
}

pub fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..12])
}

pub async fn insert_site(db: &DatabaseConnection, mode: &str, port: Option<i32>) -> sites::Model {
    let transaction = relaxed(db).await;
    let site = sites::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(Uuid::new_v4()),
        name: Set(unique("site")),
        mode: Set(mode.to_string()),
        repo_url: Set("https://example.com/org/repo".to_string()),
        production_branch: Set("main".to_string()),
        root_dir: Set(String::new()),
        container_port: Set(port),
        webhook_secret: Set("hook".to_string()),
        preview_enabled: Set(true),
        created_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    }
    .insert(&transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    site
}

pub async fn insert_deployment(
    db: &DatabaseConnection,
    site_id: Uuid,
    status: &str,
    environment: &str,
    branch: &str,
) -> site_deployments::Model {
    let model = site_deployments::ActiveModel {
        id: Set(Uuid::new_v4()),
        site_id: Set(site_id),
        commit_sha: Set("abc123".to_string()),
        commit_message: Set(String::new()),
        branch: Set(branch.to_string()),
        environment: Set(environment.to_string()),
        status: Set(status.to_string()),
        build_log: Set(String::new()),
        created_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    model.insert(db).await.unwrap()
}

pub async fn insert_domain(
    db: &DatabaseConnection,
    site_id: Uuid,
    kind: &str,
    branch: Option<&str>,
) -> String {
    let hostname = format!("{}.test", unique("host"));
    site_domains::ActiveModel {
        id: Set(Uuid::new_v4()),
        site_id: Set(site_id),
        hostname: Set(hostname.clone()),
        kind: Set(kind.to_string()),
        branch: Set(branch.map(str::to_string)),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(db)
    .await
    .unwrap();
    hostname
}

pub async fn point_site_at(db: &DatabaseConnection, site: sites::Model, deployment: Uuid) {
    let mut active: sites::ActiveModel = site.into();
    active.current_deployment_id = Set(Some(deployment));
    active.update(db).await.unwrap();
}

pub struct GitFixture {
    pub url: String,
    directory: PathBuf,
}

impl Drop for GitFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).ok();
    }
}

async fn git(directory: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.email=ci@csfx.test", "-c", "user.name=ci"])
        .args(args)
        .current_dir(directory)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

async fn cgi(State(root): State<PathBuf>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let payload = body.collect().await.unwrap().to_bytes();
    let exec = Command::new("git")
        .arg("--exec-path")
        .output()
        .await
        .unwrap();
    let backend =
        PathBuf::from(String::from_utf8_lossy(&exec.stdout).trim()).join("git-http-backend");
    let content_type = parts
        .headers
        .get("content-type")
        .map_or("", |v| v.to_str().unwrap());
    let mut command = Command::new(backend);
    command
        .env_clear()
        .env("GIT_PROJECT_ROOT", root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("REQUEST_METHOD", parts.method.as_str())
        .env("PATH_INFO", parts.uri.path())
        .env("QUERY_STRING", parts.uri.query().unwrap_or(""))
        .env("CONTENT_TYPE", content_type)
        .env("CONTENT_LENGTH", payload.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(encoding) = parts.headers.get("content-encoding") {
        command.env("HTTP_CONTENT_ENCODING", encoding.to_str().unwrap());
    }
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&payload)
        .await
        .unwrap();
    cgi_response(&child.wait_with_output().await.unwrap().stdout)
}

fn cgi_response(output: &[u8]) -> Response {
    let split = output.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let mut response = Response::builder();
    for line in String::from_utf8_lossy(&output[..split]).lines() {
        let (name, value) = line.split_once(": ").unwrap();
        response = match name {
            "Status" => response.status(value.split(' ').next().unwrap().parse::<u16>().unwrap()),
            _ => response.header(name, value),
        };
    }
    response
        .body(Body::from(output[split + 4..].to_vec()))
        .unwrap()
}

pub async fn git_fixture(files: &[(&str, &str)]) -> GitFixture {
    let directory = std::env::temp_dir().join(unique("csfx-sb-git"));
    let work = directory.join("work");
    std::fs::create_dir_all(&work).unwrap();
    for (name, content) in files {
        std::fs::write(work.join(name), content).unwrap();
    }
    git(&work, &["init", "-q", "-b", "main"]).await;
    git(&work, &["add", "."]).await;
    git(&work, &["commit", "-q", "-m", "initial"]).await;
    git(
        &directory,
        &["clone", "-q", "--bare", "work", "served/repo.git"],
    )
    .await;
    let router = Router::new()
        .fallback(any(cgi))
        .with_state(directory.join("served"));
    let address = serve(router).await;
    GitFixture {
        url: format!("http://{address}/repo.git"),
        directory,
    }
}

pub fn job_body(site: &sites::Model, deployment: &site_deployments::Model) -> Value {
    json!({
        "deployment_id": deployment.id,
        "site_id": site.id,
        "resource_group_id": site.resource_group_id,
        "site_name": site.name,
        "mode": site.mode,
        "repo_url": "https://example.com/org/repo",
        "branch": deployment.branch,
        "container_port": site.container_port,
        "env": { "API_URL": "https://api.test" },
    })
}

pub async fn deployment_of(db: &DatabaseConnection, id: Uuid) -> site_deployments::Model {
    entity::SiteDeployments::find_by_id(id)
        .one(db)
        .await
        .unwrap()
        .unwrap()
}

pub async fn site_of(db: &DatabaseConnection, id: Uuid) -> sites::Model {
    entity::Sites::find_by_id(id)
        .one(db)
        .await
        .unwrap()
        .unwrap()
}

pub async fn routes_for(
    db: &DatabaseConnection,
    hostname: &str,
) -> Vec<entity::ingress_routes::Model> {
    entity::ingress_routes::Entity::find()
        .filter(entity::ingress_routes::Column::Hostname.eq(hostname))
        .all(db)
        .await
        .unwrap()
}

pub async fn mark(
    db: &DatabaseConnection,
    deployment: site_deployments::Model,
    change: impl FnOnce(&mut site_deployments::ActiveModel),
) -> site_deployments::Model {
    let mut active: site_deployments::ActiveModel = deployment.into();
    change(&mut active);
    active.update(db).await.unwrap()
}
