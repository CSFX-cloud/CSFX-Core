use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{call, new_bucket, router};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn key_lifecycle_creates_rotates_and_deletes() {
    let app = router().await;
    let bucket = new_bucket(&app).await;
    let keys_uri = format!("/buckets/{}/keys", bucket);

    let (status, created) = call(&app, Method::POST, &keys_uri, Some(json!({"name": "ci"}))).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["permissions"], "readwrite");
    assert_eq!(created["secret_access_key"], "mock-secret");
    let key_id = created["id"].as_str().unwrap().to_string();

    let (_, listed) = call(&app, Method::GET, &keys_uri, None).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0].get("secret_access_key").is_none());

    let (status, rotated) = call(
        &app,
        Method::POST,
        &format!("{}/{}/rotate", keys_uri, key_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(rotated["id"], key_id);
    assert!(rotated["last_rotated_at"].is_string() || rotated["last_rotated_at"].is_null());
    let new_id = rotated["id"].as_str().unwrap().to_string();

    let (status, _) = call(
        &app,
        Method::DELETE,
        &format!("{}/{}", keys_uri, new_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, listed) = call(&app, Method::GET, &keys_uri, None).await;
    assert!(listed.as_array().unwrap().is_empty());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn key_operations_reject_unknown_resources() {
    let app = router().await;
    let bucket = new_bucket(&app).await;
    let other = new_bucket(&app).await;
    let (_, created) = call(
        &app,
        Method::POST,
        &format!("/buckets/{}/keys", bucket),
        Some(json!({"name": "scoped", "permissions": "read"})),
    )
    .await;
    let key_id = created["id"].as_str().unwrap();

    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/buckets/{}/keys", Uuid::new_v4()),
        Some(json!({"name": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for method in [Method::POST, Method::DELETE] {
        let suffix = if method == Method::POST {
            "/rotate"
        } else {
            ""
        };
        let wrong_bucket = format!("/buckets/{}/keys/{}{}", other, key_id, suffix);
        assert_eq!(
            call(&app, method.clone(), &wrong_bucket, None).await.0,
            StatusCode::NOT_FOUND
        );
        let unknown_key = format!("/buckets/{}/keys/{}{}", bucket, Uuid::new_v4(), suffix);
        assert_eq!(
            call(&app, method, &unknown_key, None).await.0,
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn key_creation_fails_for_bad_permissions_and_garage_errors() {
    let app = router().await;
    let bucket = new_bucket(&app).await;
    let uri = format!("/buckets/{}/keys", bucket);

    let (status, _) = call(
        &app,
        Method::POST,
        &uri,
        Some(json!({"name": "x", "permissions": "root"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let (status, _) = call(
        &app,
        Method::POST,
        &uri,
        Some(json!({"name": "failkey-one"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);

    for permissions in ["read", "readwrite", "owner"] {
        let (status, _) = call(
            &app,
            Method::POST,
            &uri,
            Some(json!({"name": permissions, "permissions": permissions})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{}", permissions);
    }
}
