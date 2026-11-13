use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{self, call};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn health_endpoint_reports_ok() {
    let app = common::router().await;

    let (status, _) = call(&app, Method::GET, "/health", None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn pre_registration_lifecycle() {
    let app = common::router().await;
    let name = format!("pre-{}", Uuid::new_v4().simple());

    let (status, created) = call(
        &app,
        Method::POST,
        "/admin/agents/pre-register",
        Some(json!({"name": name, "hostname": "host.test"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let agent_id = created["agent_id"].as_str().unwrap().to_string();

    let (_, pending) = call(&app, Method::GET, "/admin/agents/pending", None).await;
    assert!(pending
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == agent_id));

    let (_, tokens) = call(&app, Method::GET, "/admin/tokens", None).await;
    assert!(tokens
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["agent_id"] == agent_id));

    let uri = format!("/admin/agents/pending/{}", agent_id);
    let (status, _) = call(&app, Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_token_lifecycle() {
    let app = common::router().await;

    let (status, created) = call(
        &app,
        Method::POST,
        "/admin/bootstrap-tokens",
        Some(json!({"description": "ci", "max_uses": 3, "role": "master"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["role"], "master");
    assert_eq!(created["max_uses"], 3);
    let id = created["id"].as_str().unwrap().to_string();

    let (_, listed) = call(&app, Method::GET, "/admin/bootstrap-tokens", None).await;
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == id));

    let uri = format!("/admin/bootstrap-tokens/{}/revoke", id);
    let (status, revoked) = call(&app, Method::POST, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(revoked["message"].as_str().unwrap().contains(&id));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_token_defaults_to_worker_and_rejects_unknown_role() {
    let app = common::router().await;

    let (status, created) = call(
        &app,
        Method::POST,
        "/admin/bootstrap-tokens",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["role"], "worker");
    assert_eq!(created["max_uses"], 100);

    let (status, error) = call(
        &app,
        Method::POST,
        "/admin/bootstrap-tokens",
        Some(json!({"role": "owner"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("owner"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_queries_return_inserted_agent() {
    let db = common::migrated_database().await;
    let agent_id = common::insert_agent(&db).await;
    let app = common::router().await;

    let (status, agent) = call(
        &app,
        Method::GET,
        &format!("/admin/agents/{}", agent_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(agent["id"], agent_id.to_string());

    let (_, agents) = call(&app, Method::GET, "/admin/agents", None).await;
    assert!(agents
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == agent_id.to_string()));

    let (status, _) = call(
        &app,
        Method::GET,
        &format!("/admin/agents/{}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, statistics) = call(&app, Method::GET, "/admin/statistics", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(statistics["total"].as_u64().unwrap() >= 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn metrics_are_not_served_on_the_public_router() {
    let app = common::router().await;

    let (status, _) = call(&app, Method::GET, "/metrics", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
