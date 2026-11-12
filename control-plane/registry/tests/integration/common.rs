use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use rcgen::{
    BasicConstraints, CertificateParams, IsCa, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use registry::server::{create_router, AppState};
use registry::services::{
    api_keys::ApiKeyManager, bootstrap_tokens::BootstrapTokenManager, mgmt_ipam::MgmtIpamService,
    pki::PkiService, registry::AgentRegistry, tokens::TokenManager,
};
use sea_orm::{Database, DatabaseConnection};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

static CA_PEMS: OnceLock<(String, String)> = OnceLock::new();
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

fn ca_pems() -> &'static (String, String) {
    CA_PEMS.get_or_init(|| {
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let cert = params.self_signed(&key).unwrap();
        std::env::set_var("CSFX_CA_CERT_PEM", cert.pem());
        std::env::set_var("CSFX_CA_KEY_PEM", key.serialize_pem());
        (cert.pem(), key.serialize_pem())
    })
}

pub fn pki_service(db: DatabaseConnection) -> PkiService {
    ca_pems();
    PkiService::new(db, 24).expect("pki service must initialise")
}

pub fn app_state(db: DatabaseConnection) -> AppState {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    AppState {
        token_manager: Arc::new(TokenManager::new(db.clone())),
        bootstrap_token_manager: Arc::new(BootstrapTokenManager::new(db.clone())),
        api_key_manager: Arc::new(ApiKeyManager::new(db.clone())),
        agent_registry: Arc::new(AgentRegistry::new(db.clone())),
        pki_service: Arc::new(pki_service(db.clone())),
        mgmt_ipam: Arc::new(MgmtIpamService::new("http://127.0.0.1:1".to_string())),
        db,
        scheduler_url: "http://127.0.0.1:1".to_string(),
        gateway_url: "http://127.0.0.1:1".to_string(),
        http_client: reqwest::Client::new(),
        etcd_endpoints: "http://127.0.0.1:1".to_string(),
    }
}

pub fn with_etcd_ipam(mut state: AppState) -> AppState {
    let endpoints = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS must be set");
    state.mgmt_ipam = Arc::new(MgmtIpamService::new(endpoints));
    state
}

pub async fn router() -> Router {
    create_router(app_state(migrated_database().await))
}

pub fn csr_pem() -> String {
    csr_pem_for(&KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap())
}

pub fn csr_pem_for(key: &KeyPair) -> String {
    CertificateParams::new(vec!["agent.csfx.test".to_string()])
        .unwrap()
        .serialize_request(key)
        .unwrap()
        .pem()
        .unwrap()
}

pub async fn insert_agent(db: &DatabaseConnection) -> Uuid {
    insert_agent_with(db, "worker", None).await
}

pub async fn insert_agent_with(
    db: &DatabaseConnection,
    role: &str,
    machine_id: Option<String>,
) -> Uuid {
    let id = Uuid::new_v4();
    registry::db::agents::create(
        db,
        id,
        format!("agent-{}", id.simple()),
        format!("host-{}", id.simple()),
        None,
        "0.2.2".to_string(),
        "linux".to_string(),
        "6.1".to_string(),
        "x86_64".to_string(),
        "Online".to_string(),
        None,
        None,
        None,
        Some("10.100.0.2".to_string()),
        role.to_string(),
        machine_id,
    )
    .await
    .expect("agent must be inserted");
    id
}

pub async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
    peer: SocketAddr,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let payload = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let mut request = builder
        .header("content-type", "application/json")
        .body(payload)
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

pub async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send(
        app,
        method,
        uri,
        body,
        &[],
        "127.0.0.1:9000".parse().unwrap(),
    )
    .await
}
