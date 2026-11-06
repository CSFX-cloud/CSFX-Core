use std::sync::Arc;

use axum::http::{Method, StatusCode};
use axum::Router;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use entity::{agent_api_keys, image_registries, image_registry_tokens};
use ring::digest;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::Value;
use site_builder::registry::{router, RegistryState};
use site_builder::token::deployment_token;
use uuid::Uuid;

use crate::common::{
    config, insert_deployment, insert_site, migrated_database, registry_keys, relaxed, request,
    unique, SECRET,
};

fn sha256_hex(text: &str) -> String {
    digest::digest(&digest::SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn app() -> (Router, DatabaseConnection) {
    let db = migrated_database().await;
    let state = RegistryState {
        db: db.clone(),
        config: Arc::new(config("http://127.0.0.1:1", None, None)),
        keys: registry_keys(),
    };
    (router(state), db)
}

async fn token_call(app: &Router, user: &str, password: &str, query: &str) -> (StatusCode, Value) {
    let credentials = format!("Basic {}", STANDARD.encode(format!("{user}:{password}")));
    let uri = format!("/registry/token?{query}");
    let headers = [("authorization", credentials.as_str())];
    let (status, bytes) = request(app, Method::GET, &uri, &headers, Vec::new()).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn granted(response: &Value) -> Value {
    let token = response["token"].as_str().unwrap();
    let payload = token.split('.').nth(1).unwrap();
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .unwrap();
    serde_json::from_slice::<Value>(&decoded).unwrap()["access"].clone()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn certificate_endpoint_returns_the_pem() {
    let (app, _) = app().await;
    let (status, bytes) =
        request(&app, Method::GET, "/registry/certificate", &[], Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8(bytes)
        .unwrap()
        .contains("BEGIN CERTIFICATE"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn missing_or_malformed_credentials_are_unauthorized() {
    let (app, _) = app().await;
    let (status, _) = request(&app, Method::GET, "/registry/token", &[], Vec::new()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        token_call(&app, "stranger", "x", "").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        token_call(&app, "d-not-a-uuid", "x", "").await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn building_deployments_may_push_to_their_own_site_only() {
    let (app, db) = app().await;
    let site = insert_site(&db, "container", Some(3000)).await;
    let deployment = insert_deployment(&db, site.id, "building", "production", "main").await;
    let user = format!("d-{}", deployment.id);
    let password = deployment_token(SECRET, deployment.id);
    let scope = format!("service=csfx-site-registry&account=builder&scope=repository:{}:pull,push&scope=repository:other:pull", site.id);
    let (status, body) = token_call(&app, &user, &password, &scope).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = granted(&body);
    assert_eq!(access.as_array().unwrap().len(), 1);
    assert_eq!(access[0]["actions"], serde_json::json!(["pull", "push"]));
    assert_eq!(
        token_call(&app, &user, "00", "").await.0,
        StatusCode::UNAUTHORIZED
    );
    let wrong_service = token_call(&app, &user, &password, "service=other").await;
    assert_eq!(wrong_service.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn finished_deployments_cannot_authenticate() {
    let (app, db) = app().await;
    let site = insert_site(&db, "container", Some(3000)).await;
    let deployment = insert_deployment(&db, site.id, "ready", "production", "main").await;
    let user = format!("d-{}", deployment.id);
    let password = deployment_token(SECRET, deployment.id);
    assert_eq!(
        token_call(&app, &user, &password, "").await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_keys_receive_pull_access() {
    let (app, db) = app().await;
    let key = unique("agent-key");
    let transaction = relaxed(&db).await;
    agent_api_keys::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(Uuid::new_v4()),
        key_hash: Set(sha256_hex(&key)),
        created_at: Set(Utc::now().naive_utc()),
        last_used_at: Set(None),
        is_active: Set(true),
    }
    .insert(&transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let (status, body) =
        token_call(&app, "agent", &key, "scope=repository:any/app:pull,push").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(granted(&body)[0]["actions"], serde_json::json!(["pull"]));
    assert_eq!(
        token_call(&app, "agent", "unknown", "").await.0,
        StatusCode::UNAUTHORIZED
    );
}

async fn registry_token(db: &DatabaseConnection, scope: &str) -> (Uuid, String, String) {
    let transaction = relaxed(db).await;
    let group = Uuid::new_v4();
    let registry = image_registries::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group),
        name: Set(unique("reg")),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&transaction)
    .await
    .unwrap();
    let password = unique("registry-secret");
    let token = image_registry_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        registry_id: Set(registry.id),
        name: Set("ci".to_string()),
        token_hash: Set(sha256_hex(&password)),
        scope: Set(scope.to_string()),
        created_at: Set(Utc::now().naive_utc()),
        last_used_at: Set(None),
    }
    .insert(&transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let prefix = site_builder::images::registry_prefix(group, &registry.name);
    (token.id, password, prefix)
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registry_tokens_are_confined_to_their_namespace() {
    let (app, db) = app().await;
    let (id, password, prefix) = registry_token(&db, "push").await;
    let user = format!("rt-{id}");
    let scope =
        format!("scope=repository:{prefix}app:pull,push&scope=repository:elsewhere/app:pull");
    let (status, body) = token_call(&app, &user, &password, &scope).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let access = granted(&body);
    assert_eq!(access.as_array().unwrap().len(), 1);
    assert_eq!(access[0]["actions"], serde_json::json!(["pull", "push"]));
    assert_eq!(
        token_call(&app, &user, "wrong", "").await.0,
        StatusCode::UNAUTHORIZED
    );
    let unknown = format!("rt-{}", Uuid::new_v4());
    assert_eq!(
        token_call(&app, &unknown, &password, "").await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn pull_registry_tokens_never_receive_push() {
    let (app, db) = app().await;
    let (id, password, prefix) = registry_token(&db, "pull").await;
    let scope = format!("scope=repository:{prefix}app:pull,push");
    let (status, body) = token_call(&app, &format!("rt-{id}"), &password, &scope).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(granted(&body)[0]["actions"], serde_json::json!(["pull"]));
}
