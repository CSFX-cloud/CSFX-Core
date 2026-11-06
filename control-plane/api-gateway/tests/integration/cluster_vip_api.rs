use axum::http::{Method, StatusCode};
use entity::entities::system_settings;
use sea_orm::EntityTrait;
use serde_json::json;

use crate::common::harness;
use crate::support_a::{insert_agent, insert_api_key, keyed};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vip_routes_require_authorization() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    for method in [Method::GET, Method::PUT] {
        let body = (method == Method::PUT).then(|| json!({}));
        let (status, _) = harness
            .send(
                method.clone(),
                "/api/cluster/vip",
                Some(&limited),
                body.clone(),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = harness.send(method, "/api/cluster/vip", None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = harness
        .send(Method::GET, "/api/cluster/masters/vip", None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vip_is_validated_stored_and_served_to_masters() {
    let harness = harness().await;
    for invalid in [
        json!({"address": "not-an-ip", "prefix_length": 24, "router_id": 5}),
        json!({"address": "127.0.0.1", "prefix_length": 24, "router_id": 5}),
        json!({"address": "10.9.9.9", "prefix_length": 4, "router_id": 5}),
        json!({"address": "10.9.9.9", "prefix_length": 24, "router_id": 0}),
    ] {
        let (status, _) = harness
            .admin(Method::PUT, "/api/cluster/vip", Some(invalid))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    let vip = json!({"address": "10.9.9.9", "prefix_length": 24, "router_id": 5});
    let (status, body) = harness
        .admin(Method::PUT, "/api/cluster/vip", Some(vip.clone()))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["vip"], vip);
    let (_, read) = harness.get("/api/cluster/vip").await;
    assert_eq!(read["vip"], vip);

    let master = insert_agent(&harness.db, "master", "Online").await;
    let master_key = insert_api_key(&harness.db, master.id).await;
    let (status, served) = keyed(
        &harness,
        Method::GET,
        "/api/cluster/masters/vip",
        &master_key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(served, vip);

    let worker = insert_agent(&harness.db, "worker", "Online").await;
    let worker_key = insert_api_key(&harness.db, worker.id).await;
    let (status, _) = keyed(
        &harness,
        Method::GET,
        "/api/cluster/masters/vip",
        &worker_key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    system_settings::Entity::delete_by_id("cluster.vip")
        .exec(&harness.db)
        .await
        .unwrap();
}
