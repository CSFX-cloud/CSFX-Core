use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::{call, insert_agent, migrated_database, router, service};

fn unique(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4().simple())
}

async fn create(app: &axum::Router, name: &str) -> Value {
    let (status, body) = call(
        app,
        Method::POST,
        "/volumes",
        Some(json!({ "name": name, "size_gb": 4 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

fn path(volume: &Value, suffix: &str) -> String {
    format!("/volumes/{}{}", volume["id"].as_str().unwrap(), suffix)
}

#[tokio::test]
async fn health_and_metrics_respond() {
    let app = router(false).await;
    volume_manager::metrics::init();
    volume_manager::metrics::record_request("GET", "/health", 200, 0.01);
    let (status, _) = call(&app, Method::GET, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, Method::GET, "/metrics", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn create_get_list_delete_volume() {
    let app = router(false).await;
    let name = unique("vol");
    let volume = create(&app, &name).await;
    assert_eq!(volume["name"], name);
    assert_eq!(volume["status"], "available");
    assert_eq!(volume["pool"], "csfx-volumes");

    let (status, fetched) = call(&app, Method::GET, &path(&volume, ""), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["id"], volume["id"]);

    let (status, listed) = call(&app, Method::GET, "/volumes", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == volume["id"]));

    let (status, _) = call(&app, Method::DELETE, &path(&volume, ""), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = call(&app, Method::GET, &path(&volume, ""), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "volume not found");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn custom_pool_is_kept() {
    let app = router(false).await;
    let (status, body) = call(
        &app,
        Method::POST,
        "/volumes",
        Some(json!({ "name": unique("pool"), "size_gb": 1, "pool": "custom-pool" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["pool"], "custom-pool");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn resource_group_id_is_stored() {
    let app = router(false).await;
    let group = Uuid::new_v4();
    let (status, body) = call(
        &app,
        Method::POST,
        "/volumes",
        Some(json!({ "name": unique("rg"), "size_gb": 1, "resource_group_id": group })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["resource_group_id"], group.to_string());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn malformed_requests_are_rejected() {
    let app = router(false).await;
    let (status, _) = call(&app, Method::POST, "/volumes", Some(json!({ "name": 1 }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = call(&app, Method::GET, "/volumes/not-a-uuid", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn missing_volume_maps_to_server_error() {
    let app = router(false).await;
    let missing = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let (status, body) = call(&app, Method::DELETE, &format!("/volumes/{missing}"), None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().contains("not found"));
    let attach = json!({ "agent_id": agent });
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/volumes/{missing}/attach"),
        Some(attach),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/volumes/{missing}/detach"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/volumes/{missing}/snapshots"),
        Some(json!({ "name": "snap" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn attach_and_detach_volume() {
    let app = router(false).await;
    let agent = insert_agent(&migrated_database().await).await;
    let volume = create(&app, &unique("att")).await;

    let (status, attached) = call(
        &app,
        Method::POST,
        &path(&volume, "/attach"),
        Some(json!({ "agent_id": agent })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(attached["status"], "inuse");
    assert_eq!(attached["attached_to_agent"], agent.to_string());

    let (status, detached) = call(&app, Method::POST, &path(&volume, "/detach"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detached["status"], "available");
    assert!(detached["attached_to_agent"].is_null());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn attach_with_unknown_agent_fails() {
    let app = router(false).await;
    let volume = create(&app, &unique("bad-agent")).await;
    let (status, _) = call(
        &app,
        Method::POST,
        &path(&volume, "/attach"),
        Some(json!({ "agent_id": Uuid::new_v4() })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn snapshots_are_created_and_listed() {
    let app = router(false).await;
    let volume = create(&app, &unique("snap")).await;
    let (status, snap) = call(
        &app,
        Method::POST,
        &path(&volume, "/snapshots"),
        Some(json!({ "name": "nightly" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(snap["name"], "nightly");
    assert_eq!(snap["status"], "available");

    let (status, listed) = call(&app, Method::GET, &path(&volume, "/snapshots"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], snap["id"]);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detach_all_for_agent_releases_every_volume() {
    let app = router(false).await;
    let agent = insert_agent(&migrated_database().await).await;
    let mut ids = Vec::new();
    for index in 0..2 {
        let volume = create(&app, &unique(&format!("bulk{index}"))).await;
        let attach = json!({ "agent_id": agent });
        call(&app, Method::POST, &path(&volume, "/attach"), Some(attach)).await;
        ids.push(volume["id"].clone());
    }
    let uri = format!("/internal/agents/{agent}/detach-all");
    let (status, body) = call(&app, Method::POST, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    let detached = body["detached_volume_ids"].as_array().unwrap();
    assert_eq!(detached.len(), 2);
    assert!(ids.iter().all(|id| detached.contains(id)));

    let (_, again) = call(&app, Method::POST, &uri, None).await;
    assert!(again["detached_volume_ids"].as_array().unwrap().is_empty());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ceph_backed_volume_lifecycle() {
    let app = router(true).await;
    let volume = create(&app, &unique("ceph")).await;
    let (status, _) = call(
        &app,
        Method::POST,
        &path(&volume, "/snapshots"),
        Some(json!({ "name": "s1" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = call(&app, Method::DELETE, &path(&volume, ""), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ceph_create_failure_rolls_back_database_row() {
    let app = router(true).await;
    let name = unique("failcreate");
    let (status, body) = call(
        &app,
        Method::POST,
        "/volumes",
        Some(json!({ "name": name, "size_gb": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().contains("create RBD image"));
    let (_, listed) = call(&app, Method::GET, "/volumes", None).await;
    assert!(!listed
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["name"] == name));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ceph_delete_skips_missing_image_and_reports_failures() {
    let app = router(true).await;
    let missing = create(&app, &unique("missing")).await;
    let (status, _) = call(&app, Method::DELETE, &path(&missing, ""), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let busy = create(&app, &unique("failrm")).await;
    let (status, body) = call(&app, Method::DELETE, &path(&busy, ""), None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().contains("delete RBD image"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ceph_snapshot_failure_is_reported() {
    let app = router(true).await;
    let volume = create(&app, &unique("failsnap")).await;
    let (status, body) = call(
        &app,
        Method::POST,
        &path(&volume, "/snapshots"),
        Some(json!({ "name": "failsnap" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().contains("snapshot"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detach_all_fences_watchers_before_detaching() {
    let app = router(true).await;
    let agent = insert_agent(&migrated_database().await).await;
    let volume = create(&app, &unique("watched")).await;
    let attach = json!({ "agent_id": agent });
    call(&app, Method::POST, &path(&volume, "/attach"), Some(attach)).await;
    let uri = format!("/internal/agents/{agent}/detach-all");
    let (status, body) = call(&app, Method::POST, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detached_volume_ids"][0], volume["id"]);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detach_all_aborts_when_fencing_fails() {
    let app = router(true).await;
    let agent = insert_agent(&migrated_database().await).await;
    let uri = format!("/internal/agents/{agent}/detach-all");
    for name in ["watchedbad", "nostatus"] {
        let volume = create(&app, &unique(name)).await;
        let attach = json!({ "agent_id": agent });
        call(&app, Method::POST, &path(&volume, "/attach"), Some(attach)).await;
        let (status, body) = call(&app, Method::POST, &uri, None).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
        let (_, current) = call(&app, Method::GET, &path(&volume, ""), None).await;
        assert_eq!(current["status"], "inuse");
        call(&app, Method::DELETE, &path(&volume, ""), None).await;
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn service_get_volume_returns_none_for_unknown_id() {
    let service = service(migrated_database().await, None).await;
    assert!(service.get_volume(Uuid::new_v4()).await.unwrap().is_none());
    assert!(service
        .list_snapshots(Uuid::new_v4())
        .await
        .unwrap()
        .is_empty());
}
