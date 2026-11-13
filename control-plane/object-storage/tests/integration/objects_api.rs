use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{call, create_named_bucket, new_bucket, router};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn objects_are_listed_with_folders() {
    let app = router().await;
    let bucket = new_bucket(&app).await;

    let (status, listing) = call(
        &app,
        Method::GET,
        &format!("/buckets/{}/objects?prefix=docs%2F", bucket),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(listing["objects"].as_array().unwrap().len(), 1);
    assert_eq!(listing["objects"][0]["key"], "a.txt");
    assert_eq!(listing["objects"][0]["size"], 12);
    assert_eq!(listing["folders"][0], "docs/");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn objects_can_be_deleted() {
    let app = router().await;
    let bucket = new_bucket(&app).await;

    let (status, _) = call(
        &app,
        Method::DELETE,
        &format!("/buckets/{}/objects/folder/a.txt", bucket),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn presigned_urls_carry_a_signature() {
    let app = router().await;
    let bucket = new_bucket(&app).await;

    let (status, upload) = call(
        &app,
        Method::POST,
        &format!("/buckets/{}/objects/presign-upload", bucket),
        Some(json!({"key": "dir/file name.txt"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(upload["expires_in_seconds"], 900);
    let url = upload["url"].as_str().unwrap();
    assert!(url.starts_with("http://public.csfx.test/s3data/"));
    assert!(url.contains("X-Amz-Signature"));

    let (status, download) = call(
        &app,
        Method::GET,
        &format!("/buckets/{}/objects/presign-download/dir/file.txt", bucket),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(download["url"]
        .as_str()
        .unwrap()
        .contains("X-Amz-Expires=900"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn object_operations_reject_unknown_buckets() {
    let app = router().await;
    let id = Uuid::new_v4();

    let (status, _) = call(&app, Method::GET, &format!("/buckets/{}/objects", id), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        Method::DELETE,
        &format!("/buckets/{}/objects/a.txt", id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/buckets/{}/objects/presign-upload", id),
        Some(json!({"key": "a"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        Method::GET,
        &format!("/buckets/{}/objects/presign-download/a", id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn storage_backend_failures_surface_as_bad_gateway() {
    let app = router().await;
    let (status, bucket) =
        create_named_bucket(&app, &format!("s3fail-{}", Uuid::new_v4().simple())).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = bucket["id"].as_str().unwrap();

    let (status, _) = call(&app, Method::GET, &format!("/buckets/{}/objects", id), None).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let (status, _) = call(
        &app,
        Method::DELETE,
        &format!("/buckets/{}/objects/a.txt", id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn listings_beyond_one_page_expose_the_continuation_token() {
    let app = router().await;
    let (status, bucket) =
        create_named_bucket(&app, &format!("paged-{}", Uuid::new_v4().simple())).await;
    assert_eq!(status, StatusCode::CREATED);
    let uri = format!("/buckets/{}/objects", bucket["id"].as_str().unwrap());

    let mut total = 0;
    let mut token: Option<String> = None;
    loop {
        let page_uri = match &token {
            Some(value) => format!("{}?continuation_token={}", uri, value),
            None => uri.clone(),
        };
        let (status, page) = call(&app, Method::GET, &page_uri, None).await;
        assert_eq!(status, StatusCode::OK);
        total += page["objects"].as_array().unwrap().len();
        token = page["next_continuation_token"].as_str().map(str::to_string);
        if token.is_none() {
            break;
        }
    }

    assert_eq!(total, 1500);
}
