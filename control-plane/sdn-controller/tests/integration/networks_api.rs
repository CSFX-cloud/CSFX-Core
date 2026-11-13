use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{call, create_network, router};

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn health_is_counted_and_metrics_are_not_public() {
    let app = router().await;

    let (health, _) = call(&app, Method::GET, "/health", None).await;
    let (metrics, _) = call(&app, Method::GET, "/metrics", None).await;

    assert_eq!(health, StatusCode::OK);
    assert_eq!(metrics, StatusCode::NOT_FOUND);
    assert!(String::from_utf8(shared::observability::render())
        .unwrap()
        .contains("csfx_http_requests_total"));
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn network_lifecycle() {
    let app = router().await;
    let id = create_network(&app, "10.77.0.0/24").await;

    let (status, network) = call(&app, Method::GET, &format!("/networks/{}", id), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(network["cidr"], "10.77.0.0/24");
    assert_eq!(network["status"], "active");

    let (_, networks) = call(&app, Method::GET, "/networks", None).await;
    assert!(networks
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == id));

    let (status, _) = call(&app, Method::DELETE, &format!("/networks/{}", id), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, error) = call(&app, Method::GET, &format!("/networks/{}", id), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"], "Network not found");
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn policies_are_stored_per_network() {
    let app = router().await;
    let id = create_network(&app, "10.78.0.0/24").await;
    let other = create_network(&app, "10.79.0.0/24").await;

    let (status, policy) = call(
        &app,
        Method::POST,
        &format!("/networks/{}/policies", id),
        Some(json!({
            "direction": "ingress",
            "action": "allow",
            "source_cidr": "10.0.0.0/8",
            "destination_cidr": null,
            "port": 443,
            "protocol": "tcp",
            "priority": 10
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(policy["port"], 443);

    let (_, policies) = call(
        &app,
        Method::GET,
        &format!("/networks/{}/policies", id),
        None,
    )
    .await;
    assert_eq!(policies.as_array().unwrap().len(), 1);
    let (_, none) = call(
        &app,
        Method::GET,
        &format!("/networks/{}/policies", other),
        None,
    )
    .await;
    assert!(none.as_array().unwrap().is_empty());
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn members_receive_sequential_addresses_that_are_reused() {
    let app = router().await;
    let id = create_network(&app, "10.80.0.0/24").await;
    let members_uri = format!("/networks/{}/members", id);
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    let (status, member) = call(
        &app,
        Method::POST,
        &members_uri,
        Some(json!({"workload_id": first})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(member["allocated_ip"], "10.80.0.2");
    let (_, member) = call(
        &app,
        Method::POST,
        &members_uri,
        Some(json!({"workload_id": second})),
    )
    .await;
    assert_eq!(member["allocated_ip"], "10.80.0.3");

    let (_, members) = call(&app, Method::GET, &members_uri, None).await;
    assert_eq!(members.as_array().unwrap().len(), 2);

    let (status, _) = call(
        &app,
        Method::DELETE,
        &format!("{}/{}", members_uri, first),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, members) = call(&app, Method::GET, &members_uri, None).await;
    assert_eq!(members.as_array().unwrap().len(), 1);

    let (_, member) = call(
        &app,
        Method::POST,
        &members_uri,
        Some(json!({"workload_id": Uuid::new_v4()})),
    )
    .await;
    assert_eq!(member["allocated_ip"], "10.80.0.2");
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn member_allocation_fails_for_unknown_exhausted_or_invalid_networks() {
    let app = router().await;
    let workload = || Some(json!({"workload_id": Uuid::new_v4()}));

    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/networks/{}/members", Uuid::new_v4()),
        workload(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let small = create_network(&app, "10.81.0.0/30").await;
    let uri = format!("/networks/{}/members", small);
    let (status, _) = call(&app, Method::POST, &uri, workload()).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, error) = call(&app, Method::POST, &uri, workload()).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(error["error"].as_str().unwrap().contains("exhausted"));

    for cidr in ["not-a-cidr", "10.82.0.0/abc", "bogus/24"] {
        let invalid = create_network(&app, cidr).await;
        let (status, _) = call(
            &app,
            Method::POST,
            &format!("/networks/{}/members", invalid),
            workload(),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{}", cidr);
    }
}
