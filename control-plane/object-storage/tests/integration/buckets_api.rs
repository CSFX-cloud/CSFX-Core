use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{call, create_named_bucket, database, new_bucket, router};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn health_and_metrics_respond() {
    let app = router().await;

    assert_eq!(
        call(&app, Method::GET, "/health", None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, Method::GET, "/metrics", None).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bucket_lifecycle() {
    let app = router().await;
    let name = format!("lifecycle-{}", Uuid::new_v4().simple());

    let (status, created) = call(
        &app,
        Method::POST,
        "/buckets",
        Some(json!({"name": name, "exposure": "private", "quota_max_size": 1024, "quota_max_objects": 10})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["quota_max_size"], 1024);
    assert!(created["global_alias"].as_str().unwrap().starts_with(&name));
    let id = created["id"].as_str().unwrap().to_string();

    let (status, fetched) = call(&app, Method::GET, &format!("/buckets/{}", id), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["name"], name);

    let (_, listed) = call(&app, Method::GET, "/buckets", None).await;
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|bucket| bucket["id"] == id));

    let (status, updated) = call(
        &app,
        Method::PATCH,
        &format!("/buckets/{}", id),
        Some(json!({"exposure": "public", "quota_max_size": 2048})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["exposure"], "public");
    assert_eq!(updated["quota_max_size"], 2048);

    let (status, _) = call(&app, Method::DELETE, &format!("/buckets/{}", id), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, Method::GET, &format!("/buckets/{}", id), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bucket_listing_filters_by_group_and_organization() {
    let app = router().await;
    let group = Uuid::new_v4();
    let (status, _) = call(
        &app,
        Method::POST,
        "/buckets",
        Some(json!({"name": format!("scoped-{}", Uuid::new_v4().simple()), "resource_group_id": group})),
    )
    .await;
    assert!(status == StatusCode::CREATED || status == StatusCode::BAD_GATEWAY);

    let (status, listed) = call(
        &app,
        Method::GET,
        &format!("/buckets?resource_group_id={}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed.as_array().unwrap().is_empty());
    let (status, listed) = call(
        &app,
        Method::GET,
        &format!("/buckets?organization_id={}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed.as_array().unwrap().is_empty());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_buckets_are_not_found() {
    let app = router().await;
    let uri = format!("/buckets/{}", Uuid::new_v4());

    assert_eq!(
        call(&app, Method::GET, &uri, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, Method::DELETE, &uri, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = call(
        &app,
        Method::PATCH,
        &uri,
        Some(json!({"exposure": "public"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn garage_failures_surface_as_bad_gateway() {
    let app = router().await;

    for marker in ["failcreate", "failallow", "failkey"] {
        let (status, error) =
            create_named_bucket(&app, &format!("{}-{}", marker, Uuid::new_v4().simple())).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{}", marker);
        assert!(error["error"].is_string());
    }

    let (status, _) = call(
        &app,
        Method::POST,
        "/buckets",
        Some(
            json!({"name": format!("failquota-{}", Uuid::new_v4().simple()), "quota_max_size": 5}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);

    let (status, bucket) =
        create_named_bucket(&app, &format!("faildelete-{}", Uuid::new_v4().simple())).await;
    assert_eq!(status, StatusCode::CREATED);
    let uri = format!("/buckets/{}", bucket["id"].as_str().unwrap());
    assert_eq!(
        call(&app, Method::DELETE, &uri, None).await.0,
        StatusCode::BAD_GATEWAY
    );

    let id = new_bucket(&app).await;
    let (status, _) = call(
        &app,
        Method::PATCH,
        &format!("/buckets/{}", id),
        Some(json!({"exposure": "public"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn cluster_status_reports_replication_state() {
    let app = router().await;
    database().await;

    let (status, cluster) = call(&app, Method::GET, "/cluster", None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(cluster["replication_factor"].as_u64().unwrap() >= 1);
    assert!(cluster["nodes"].is_array());
}
