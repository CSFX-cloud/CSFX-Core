use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{call, router};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_stack_is_not_found() {
    let app = router().await;

    let (status, _) = call(
        &app,
        Method::GET,
        &format!("/workload-stacks/{}", Uuid::new_v4()),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn stack_creation_rejects_invalid_compose() {
    let app = router().await;
    let cases = [
        ("services: [", "invalid compose yaml"),
        ("services: {}", "no services"),
        (
            "services:\n  web:\n    build: .\n",
            "only image is supported",
        ),
        ("services:\n  web:\n    ports: ['80']\n", "has no image"),
    ];

    for (yaml, expected) in cases {
        let (status, error) = call(
            &app,
            Method::POST,
            "/workload-stacks",
            Some(json!({
                "name": "stack",
                "resource_group_id": Uuid::new_v4(),
                "compose_yaml": yaml
            })),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            error["error"].as_str().unwrap().contains(expected),
            "yaml {}",
            yaml
        );
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn stopping_and_restarting_empty_stack_succeeds() {
    let app = router().await;
    let id = Uuid::new_v4();

    for action in ["stop", "restart"] {
        let (status, _) = call(
            &app,
            Method::POST,
            &format!("/workload-stacks/{}/{}", id, action),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn redeploy_rejects_invalid_compose() {
    let app = router().await;

    let (status, error) = call(
        &app,
        Method::PATCH,
        &format!("/workload-stacks/{}", Uuid::new_v4()),
        Some(json!({"compose_yaml": "services: {}"})),
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(error["error"].as_str().unwrap().contains("no services"));
}
