use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::common::{call, git_fixture, harness, SECRET};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn health_is_public() {
    let harness = harness(None, None).await;
    let (status, _) = call(&harness.app, Method::GET, "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detect_requires_the_admin_secret() {
    let harness = harness(None, None).await;
    let body = json!({ "repo_url": "https://example.com/a", "branch": "main" });
    let uri = "/internal/detect";
    let (anonymous, _) = call(&harness.app, Method::POST, uri, None, Some(body.clone())).await;
    let (wrong, _) = call(&harness.app, Method::POST, uri, Some("nope"), Some(body)).await;
    assert_eq!(anonymous, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detect_rejects_unsafe_and_unreachable_repositories() {
    let harness = harness(None, None).await;
    let uri = "/internal/detect";
    let unsafe_url = json!({ "repo_url": "file:///etc", "branch": "main" });
    let (status, body) = call(
        &harness.app,
        Method::POST,
        uri,
        Some(SECRET),
        Some(unsafe_url),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("https://"));
    let unreachable =
        json!({ "repo_url": "http://127.0.0.1:1/x.git", "branch": "main", "token": "tok" });
    let (status, body) = call(
        &harness.app,
        Method::POST,
        uri,
        Some(SECRET),
        Some(unreachable),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().starts_with("git error"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detect_reads_a_cloned_repository() {
    let harness = harness(None, None).await;
    let package = r#"{"dependencies":{"vite":"5.0.0"},"scripts":{"build":"vite build"}}"#;
    let fixture = git_fixture(&[("package.json", package), ("index.html", "<html></html>")]).await;
    let request = json!({ "repo_url": fixture.url, "branch": "main", "token": "secret-token" });
    let uri = "/internal/detect";
    let (status, body) = call(&harness.app, Method::POST, uri, Some(SECRET), Some(request)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["detection"].is_object());
    let missing_root = json!({ "repo_url": fixture.url, "branch": "main", "root_dir": "nowhere" });
    let (status, _) = call(
        &harness.app,
        Method::POST,
        uri,
        Some(SECRET),
        Some(missing_root),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let missing_branch = json!({ "repo_url": fixture.url, "branch": "ghost" });
    let (status, _) = call(
        &harness.app,
        Method::POST,
        uri,
        Some(SECRET),
        Some(missing_branch),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
