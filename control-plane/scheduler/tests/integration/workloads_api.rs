use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{
    call, create_pending_workload, find_workload, router, unique_name, unplaceable_request,
};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn health_and_metrics_respond() {
    let app = router().await;

    let (health, _) = call(&app, Method::GET, "/health", None).await;
    let (metrics, _) = call(&app, Method::GET, "/metrics", None).await;

    assert_eq!(health, StatusCode::OK);
    assert_eq!(metrics, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unplaceable_workload_stays_pending() {
    let app = router().await;
    let name = unique_name("pending");

    let (status, created) = call(
        &app,
        Method::POST,
        "/workloads",
        Some(unplaceable_request(&name)),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["status"], "pending");
    assert!(created["assigned_agent_id"].is_null());
    let workload = find_workload(&app, created["workload_id"].as_str().unwrap()).await;
    assert_eq!(workload["name"], name);
    assert_eq!(workload["desired_state"], "running");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn replicas_are_created_with_numbered_names() {
    let app = router().await;
    let name = unique_name("replica");
    let mut request = unplaceable_request(&name);
    request["replicas"] = json!(2);

    let (status, created) = call(&app, Method::POST, "/workloads", Some(request)).await;

    assert_eq!(status, StatusCode::CREATED);
    let responses = created.as_array().unwrap();
    assert_eq!(responses.len(), 2);
    let first = find_workload(&app, responses[0]["workload_id"].as_str().unwrap()).await;
    let second = find_workload(&app, responses[1]["workload_id"].as_str().unwrap()).await;
    assert_eq!(first["name"], format!("{}-1", name));
    assert_eq!(second["name"], format!("{}-2", name));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn invalid_requests_are_rejected() {
    let app = router().await;
    let cases = [
        ("cpu_millicores", json!(-1)),
        ("cpu_limit_millicores", json!(0)),
        ("memory_limit_bytes", json!(0)),
        ("replicas", json!(0)),
        ("replicas", json!(33)),
    ];

    for (field, value) in cases {
        let mut request = unplaceable_request(&unique_name("invalid"));
        request["cpu_millicores"] = json!(10);
        request["memory_bytes"] = json!(10);
        request[field] = value;
        let (status, error) = call(&app, Method::POST, "/workloads", Some(request)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "field {}", field);
        assert!(error["error"].is_string());
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn stop_and_restart_change_desired_state() {
    let app = router().await;
    let id = create_pending_workload(&app).await;

    let (status, stopped) =
        call(&app, Method::POST, &format!("/workloads/{}/stop", id), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stopped["desired_state"], "stopped");
    assert_eq!(stopped["status"], "stopped");

    let (status, restarted) = call(
        &app,
        Method::POST,
        &format!("/workloads/{}/restart", id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(restarted["desired_state"], "running");
    assert_eq!(restarted["status"], "starting");
    assert_eq!(restarted["restart_requested"], true);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_workloads_return_not_found() {
    let app = router().await;
    let id = Uuid::new_v4();

    for (method, suffix, body) in [
        (Method::POST, "/stop", None),
        (Method::POST, "/restart", None),
        (Method::PATCH, "", Some(json!({"image": "redis"}))),
    ] {
        let (status, _) = call(&app, method, &format!("/workloads/{}{}", id, suffix), body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn update_applies_spec_changes() {
    let app = router().await;
    let id = create_pending_workload(&app).await;

    let (status, updated) = call(
        &app,
        Method::PATCH,
        &format!("/workloads/{}", id),
        Some(json!({
            "image": "redis:7",
            "env_vars": {"MODE": "test"},
            "ports": [{"container_port": 6379}],
            "restart_policy": "never",
            "max_restarts": 3
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["image"], "redis:7");
    assert_eq!(updated["restart_policy"], "never");
    assert_eq!(updated["max_restarts"], 3);
    let workload = find_workload(&app, &id).await;
    assert_eq!(workload["env_vars"]["MODE"], "test");
    assert_eq!(workload["ports"][0]["container_port"], 6379);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn deleting_unknown_workload_fails() {
    let app = router().await;

    let (status, error) = call(
        &app,
        Method::DELETE,
        &format!("/workloads/{}", Uuid::new_v4()),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("Failed to delete workload"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn hostile_service_names_are_rejected() {
    let app = router().await;
    let name = unique_name("zone");
    let mut request = unplaceable_request(&name);
    request["service_name"] = json!("web\n$INCLUDE /var/lib/csfx-agent/dns/other.zone");

    let (status, _) = call(&app, Method::POST, "/workloads", Some(request)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, listed) = call(&app, Method::GET, "/workloads", None).await;
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .all(|workload| workload["name"] != name));
}
