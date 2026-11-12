use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::{
    call, insert_bucket_in_group, insert_resource_group, insert_volume_in_group, migrated_database,
    router, unique_name, unplaceable_request,
};

fn request_in(group: Uuid, extra: Value) -> Value {
    let mut request = unplaceable_request(&unique_name("guarded"));
    request["resource_group_id"] = json!(group);
    request.as_object_mut().unwrap().extend(
        extra
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    request
}

fn mount(volume: Uuid) -> Value {
    json!({"volume_mounts": [{"volume_id": volume, "mount_path": "/data"}]})
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn foreign_volume_is_rejected() {
    let app = router().await;
    let db = migrated_database().await;
    let own = insert_resource_group().await;
    let other = insert_resource_group().await;
    let foreign = insert_volume_in_group(&db, Some(other)).await;
    let unassigned = insert_volume_in_group(&db, None).await;

    for volume in [foreign, unassigned] {
        let request = request_in(own, mount(volume));
        let (status, _) = call(&app, Method::POST, "/workloads", Some(request)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn own_volume_is_accepted() {
    let app = router().await;
    let db = migrated_database().await;
    let own = insert_resource_group().await;
    let volume = insert_volume_in_group(&db, Some(own)).await;

    let request = request_in(own, mount(volume));
    let (status, _) = call(&app, Method::POST, "/workloads", Some(request)).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn foreign_bucket_is_rejected() {
    let app = router().await;
    let db = migrated_database().await;
    let own = insert_resource_group().await;
    let other = insert_resource_group().await;
    let foreign = insert_bucket_in_group(&db, Some(other)).await;

    let request = request_in(
        own,
        json!({"bucket_bindings": [{"bucket_id": foreign, "permissions": "read"}]}),
    );
    let (status, _) = call(&app, Method::POST, "/workloads", Some(request)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unsupported_bucket_permissions_are_rejected() {
    let app = router().await;
    let db = migrated_database().await;
    let own = insert_resource_group().await;
    let bucket = insert_bucket_in_group(&db, Some(own)).await;

    let request = request_in(
        own,
        json!({"bucket_bindings": [{"bucket_id": bucket, "permissions": "owner"}]}),
    );
    let (status, _) = call(&app, Method::POST, "/workloads", Some(request)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn foreign_volume_cannot_be_mounted_by_update() {
    let app = router().await;
    let db = migrated_database().await;
    let own = insert_resource_group().await;
    let other = insert_resource_group().await;
    let (status, created) = call(
        &app,
        Method::POST,
        "/workloads",
        Some(request_in(own, json!({}))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let foreign = insert_volume_in_group(&db, Some(other)).await;

    let uri = format!("/workloads/{}", created["workload_id"].as_str().unwrap());
    let (status, _) = call(&app, Method::PATCH, &uri, Some(mount(foreign))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
