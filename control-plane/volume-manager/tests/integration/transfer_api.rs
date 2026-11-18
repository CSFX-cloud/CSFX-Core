use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use entity::entities::volumes;
use http_body_util::BodyExt;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;

use crate::common::{call, migrated_database, router};

const IMAGE_BYTES: usize = 2_500_000;

fn scratch(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("csfx-{prefix}-{}", Uuid::new_v4()))
}

async fn raw(app: &Router, method: Method, uri: &str, body: Body) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(body)
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

async fn eventually_logged(log: &PathBuf, needle: &str) -> bool {
    for _ in 0..50 {
        let content = std::fs::read_to_string(log).unwrap_or_default();
        if content.contains(needle) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn export_streams_the_image_and_removes_its_snapshot() {
    let log = scratch("rbd-log");
    std::env::set_var("FAKE_RBD_LOG", &log);
    let app = router(true).await;
    let name = format!("export-{}", Uuid::new_v4().simple());
    let (status, volume) = call(
        &app,
        Method::POST,
        "/volumes",
        Some(json!({ "name": name, "size_gb": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = volume["id"].as_str().unwrap();

    let (status, bytes) = raw(
        &app,
        Method::GET,
        &format!("/volumes/{id}/export"),
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes.len(), IMAGE_BYTES);
    assert!(bytes.starts_with(b"csfx\ncsfx\n"));

    let content = std::fs::read_to_string(&log).unwrap();
    let snapshot_line = content
        .lines()
        .find(|line| line.contains(" snap create ") && line.contains("@csfx-export-"))
        .expect("export must snapshot the image first");
    let snapshot = snapshot_line.rsplit('@').next().unwrap();
    assert!(
        eventually_logged(&log, "snap rm ").await
            && eventually_logged(&log, &format!("@{snapshot}")).await
    );
    let removed = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .any(|line| line.contains(" snap rm ") && line.ends_with(&format!("@{snapshot}")));
    assert!(removed, "export snapshot must be deleted afterwards");
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn export_of_an_unknown_volume_is_not_found() {
    let app = router(true).await;
    let (status, _) = raw(
        &app,
        Method::GET,
        &format!("/volumes/{}/export", Uuid::new_v4()),
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn import_creates_an_available_volume_from_the_stream() {
    let received = scratch("rbd-import");
    std::env::set_var("FAKE_RBD_IMPORT_BYTES", &received);
    let app = router(true).await;
    let name = format!("restored-{}", Uuid::new_v4().simple());
    let (status, bytes) = raw(
        &app,
        Method::POST,
        &format!("/volumes/import?name={name}&size_gb=3"),
        Body::from(vec![7u8; IMAGE_BYTES]),
    )
    .await;
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["status"], "available");
    assert_eq!(body["size_gb"], 3);
    let copied = std::fs::read_to_string(&received).unwrap();
    assert_eq!(copied.trim(), IMAGE_BYTES.to_string());
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn failed_import_leaves_no_volume_behind() {
    let app = router(true).await;
    let name = format!("failimport-{}", Uuid::new_v4().simple());
    let (status, bytes) = raw(
        &app,
        Method::POST,
        &format!("/volumes/import?name={name}&size_gb=1"),
        Body::from(vec![1u8; 1000]),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["error"].as_str().unwrap().contains("import broke"));
    let db = migrated_database().await;
    let leftover = volumes::Entity::find()
        .filter(volumes::Column::Name.eq(name))
        .one(&db)
        .await
        .unwrap();
    assert!(leftover.is_none());
}
