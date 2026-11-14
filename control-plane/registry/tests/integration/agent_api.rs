use axum::http::{Method, StatusCode};
use sea_orm::EntityTrait;
use serde_json::json;
use uuid::Uuid;

use crate::common::{self, call, send};

async fn authenticated_agent() -> (axum::Router, Uuid, String) {
    let db = common::migrated_database().await;
    let agent_id = common::insert_agent(&db).await;
    let state = common::app_state(db);
    let key = state
        .api_key_manager
        .issue_key(agent_id, false)
        .await
        .unwrap()
        .key;
    (registry::server::create_router(state), agent_id, key)
}

async fn post_as_agent(
    app: &axum::Router,
    uri: &str,
    key: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        Method::POST,
        uri,
        Some(body),
        &[("X-API-Key", key)],
        "127.0.0.1:9000".parse().unwrap(),
    )
    .await
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn certificate_issue_requires_matching_api_key() {
    let (app, agent_id, key) = authenticated_agent().await;
    let uri = format!("/agents/{}/certificate", agent_id);
    let body = json!({"csr_pem": common::csr_pem()});

    let (status, _) = call(&app, Method::POST, &uri, Some(body.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = post_as_agent(&app, &uri, "csfx_agent_unknown", body.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let other = format!("/agents/{}/certificate", Uuid::new_v4());
    let (status, _) = post_as_agent(&app, &other, &key, body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn certificate_issue_rotate_and_revoke_flow() {
    let (app, agent_id, key) = authenticated_agent().await;

    let (status, issued) = post_as_agent(
        &app,
        &format!("/agents/{}/certificate", agent_id),
        &key,
        json!({"csr_pem": common::csr_pem()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let first_serial = issued["serial_number"].as_i64().unwrap();
    assert!(issued["ca_cert_pem"]
        .as_str()
        .unwrap()
        .contains("BEGIN CERTIFICATE"));

    let (status, rotated) = post_as_agent(
        &app,
        &format!("/agents/{}/rotate-certificate", agent_id),
        &key,
        json!({"new_csr_pem": common::csr_pem()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let second_serial = rotated["serial_number"].as_i64().unwrap();

    let (_, crl) = call(&app, Method::GET, "/pki/crl", None).await;
    let revoked = crl["revoked_serials"].as_array().unwrap();
    assert!(revoked.contains(&json!(first_serial)));

    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/admin/agents/{}/revoke", agent_id),
        Some(json!({"reason": "compromised"})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, crl) = call(&app, Method::GET, "/pki/crl", None).await;
    assert!(crl["revoked_serials"]
        .as_array()
        .unwrap()
        .contains(&json!(second_serial)));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn certificate_issue_rejects_invalid_csr() {
    let (app, agent_id, key) = authenticated_agent().await;

    let (status, error) = post_as_agent(
        &app,
        &format!("/agents/{}/certificate", agent_id),
        &key,
        json!({"csr_pem": "garbage"}),
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(error["error"].as_str().unwrap().contains("Invalid CSR PEM"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn revoking_without_certificate_fails() {
    let (app, agent_id, _) = authenticated_agent().await;

    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/admin/agents/{}/revoke", agent_id),
        Some(json!({"reason": "none"})),
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_endpoint_exposes_public_key_after_issue() {
    let (app, agent_id, key) = authenticated_agent().await;
    let uri = format!("/admin/agents/{}/endpoint", agent_id);

    let (status, endpoint) = call(&app, Method::GET, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(endpoint["public_key_pem"].is_null());

    post_as_agent(
        &app,
        &format!("/agents/{}/certificate", agent_id),
        &key,
        json!({"csr_pem": common::csr_pem()}),
    )
    .await;
    let (_, endpoint) = call(&app, Method::GET, &uri, None).await;
    assert!(endpoint["public_key_pem"].is_string());

    let missing = format!("/admin/agents/{}/endpoint", Uuid::new_v4());
    let (status, _) = call(&app, Method::GET, &missing, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn proxy_ticket_is_limited_to_private_networks() {
    let app = common::router().await;
    let body = json!({"agent_id": Uuid::new_v4(), "workload_id": "workload-1"});

    for (peer, expected) in [
        ("127.0.0.1:9000", StatusCode::OK),
        ("10.1.2.3:9000", StatusCode::OK),
        ("172.20.0.5:9000", StatusCode::OK),
        ("192.168.1.9:9000", StatusCode::OK),
        ("8.8.8.8:9000", StatusCode::FORBIDDEN),
        ("172.32.0.1:9000", StatusCode::FORBIDDEN),
        ("[2001:db8::1]:9000", StatusCode::FORBIDDEN),
    ] {
        let (status, _) = send(
            &app,
            Method::POST,
            "/internal/agent-proxy-ticket",
            Some(body.clone()),
            &[],
            peer.parse().unwrap(),
        )
        .await;
        assert_eq!(status, expected, "peer {}", peer);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn heartbeat_rejects_missing_unknown_and_mismatched_keys() {
    let (app, agent_id, key) = authenticated_agent().await;
    let uri = format!("/agents/{}/heartbeat", agent_id);
    let body = json!({});

    let (status, _) = call(&app, Method::POST, &uri, Some(body.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = post_as_agent(&app, &uri, "csfx_agent_unknown", body.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let other = format!("/agents/{}/heartbeat", Uuid::new_v4());
    let (status, _) = post_as_agent(&app, &other, &key, body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn heartbeat_updates_agent_state() {
    let (app, agent_id, key) = authenticated_agent().await;

    let (status, response) = post_as_agent(
        &app,
        &format!("/agents/{}/heartbeat", agent_id),
        &key,
        json!({"agent_version": "9.9.9", "wg_endpoint": "203.0.113.7:51820"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["success"], true);
    let (_, agent) = call(
        &app,
        Method::GET,
        &format!("/admin/agents/{}", agent_id),
        None,
    )
    .await;
    assert_eq!(agent["agent_version"], "9.9.9");
    assert_eq!(agent["ip_address"], "203.0.113.7");
    assert_eq!(agent["status"], "Online");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn heartbeat_cannot_claim_another_agents_mesh_identity() {
    let state = common::app_state(common::migrated_database().await);
    let victim = common::insert_agent(&state.db).await;
    let attacker = common::insert_agent(&state.db).await;
    let key = state
        .api_key_manager
        .issue_key(attacker, false)
        .await
        .unwrap()
        .key;
    let app = registry::server::create_router(state.clone());
    let victim_endpoint = format!("198.51.100.{}:51820", victim.as_u128() % 200 + 1);
    let uri = |id: Uuid| format!("/agents/{}/heartbeat", id);

    let victim_key = state
        .api_key_manager
        .issue_key(victim, false)
        .await
        .unwrap()
        .key;
    let (status, _) = post_as_agent(
        &app,
        &uri(victim),
        &victim_key,
        json!({"wg_endpoint": victim_endpoint}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = post_as_agent(
        &app,
        &uri(attacker),
        &key,
        json!({"wg_endpoint": victim_endpoint}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = post_as_agent(
        &app,
        &uri(attacker),
        &key,
        json!({"wg_tunnel_ip": "10.100.9.9", "wg_endpoint": "203.0.113.9:51820"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let stored = entity::agents::Entity::find_by_id(attacker)
        .one(&state.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.wg_tunnel_ip.as_deref(), Some("10.100.0.2"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registration_rejects_invalid_tokens() {
    let app = common::router().await;
    let request = |token: &str, name: &str| {
        json!({
            "registration_token": token,
            "name": name,
            "hostname": "host.test",
            "os_type": "linux",
            "os_version": "6.1",
            "architecture": "x86_64",
            "agent_version": "0.2.2"
        })
    };

    let (status, _) = call(
        &app,
        Method::POST,
        "/agents/register",
        Some(request("reg_unknown", "a")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = call(
        &app,
        Method::POST,
        "/agents/register",
        Some(request("csfx-bootstrap.unknown", "a")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, created) = call(
        &app,
        Method::POST,
        "/admin/agents/pre-register",
        Some(json!({"name": format!("n-{}", Uuid::new_v4().simple()), "hostname": "host.test"})),
    )
    .await;
    let token = created["registration_token"].as_str().unwrap();
    let (status, error) = call(
        &app,
        Method::POST,
        "/agents/register",
        Some(request(token, "other")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(error["error"].as_str().unwrap().contains("name mismatch"));
}

fn registration_body(token: &str, machine_id: &str, csr_pem: String) -> serde_json::Value {
    json!({
        "registration_token": token,
        "name": format!("joiner-{}", Uuid::new_v4().simple()),
        "hostname": format!("host-{}", Uuid::new_v4().simple()),
        "os_type": "linux",
        "os_version": "6.1",
        "architecture": "x86_64",
        "agent_version": "0.2.2",
        "machine_id": machine_id,
        "csr_pem": csr_pem,
    })
}

struct Victim {
    id: Uuid,
    machine_id: String,
    key: rcgen::KeyPair,
    api_key: String,
}

async fn registered_master(state: &registry::server::AppState) -> Victim {
    let machine_id = Uuid::new_v4().simple().to_string();
    let id = common::insert_agent_with(&state.db, "master", Some(machine_id.clone())).await;
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    state
        .pki_service
        .issue_certificate(id, &common::csr_pem_for(&key))
        .await
        .unwrap();
    let api_key = state
        .api_key_manager
        .issue_key(id, false)
        .await
        .unwrap()
        .key;
    Victim {
        id,
        machine_id,
        key,
        api_key,
    }
}

async fn worker_bootstrap_token(state: &registry::server::AppState) -> String {
    state
        .bootstrap_token_manager
        .create(None, "test".to_string(), 1, 5, "worker".to_string())
        .await
        .unwrap()
        .token
}

async fn register(app: &axum::Router, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    send(
        app,
        Method::POST,
        "/agents/register",
        Some(body),
        &[],
        "127.0.0.1:9000".parse().unwrap(),
    )
    .await
}

async fn role_of(state: &registry::server::AppState, agent_id: Uuid) -> String {
    entity::agents::Entity::find_by_id(agent_id)
        .one(&state.db)
        .await
        .unwrap()
        .unwrap()
        .role
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL and etcd via ETCD_ENDPOINTS"]
async fn bootstrap_registration_with_foreign_machine_id_does_not_take_over_the_agent() {
    let state = common::with_etcd_ipam(common::app_state(common::migrated_database().await));
    let victim = registered_master(&state).await;
    let token = worker_bootstrap_token(&state).await;
    let app = registry::server::create_router(state.clone());

    let body = registration_body(&token, &victim.machine_id, common::csr_pem());
    let (status, response) = register(&app, body).await;

    assert_eq!(status, StatusCode::OK);
    let new_id: Uuid = response["agent_id"].as_str().unwrap().parse().unwrap();
    assert_ne!(new_id, victim.id);
    assert_eq!(role_of(&state, victim.id).await, "master");
    assert_eq!(
        state.api_key_manager.validate_key(&victim.api_key).await,
        Ok(victim.id)
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_registration_proving_the_certificate_key_reregisters_without_changing_the_role()
{
    let state = common::app_state(common::migrated_database().await);
    let victim = registered_master(&state).await;
    let token = worker_bootstrap_token(&state).await;
    let app = registry::server::create_router(state.clone());

    let body = registration_body(&token, &victim.machine_id, common::csr_pem_for(&victim.key));
    let (status, response) = register(&app, body).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["agent_id"], victim.id.to_string());
    assert_eq!(role_of(&state, victim.id).await, "master");
    assert!(state
        .api_key_manager
        .validate_key(&victim.api_key)
        .await
        .is_err());
}
