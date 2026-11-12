use axum::http::{Method, StatusCode};
use serde_json::json;
use site_builder::git::{self, RepoPolicy, SnapshotRequest};

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
    for internal in [
        "http://127.0.0.1:2379/x",
        "https://127.0.0.1:2379/x.git",
        "https://169.254.169.254/latest",
        "https://10.0.0.5/x.git",
    ] {
        let request = json!({ "repo_url": internal, "branch": "main", "token": "tok" });
        let (status, body) =
            call(&harness.app, Method::POST, uri, Some(SECRET), Some(request)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{internal}");
        let message = body["error"].as_str().unwrap();
        assert!(
            !message.contains("127.0.0.1") && !message.contains("tok"),
            "{message}"
        );
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detect_reads_a_cloned_repository() {
    let harness = harness(None, None).await;
    let package = r#"{"dependencies":{"vite":"5.0.0"},"scripts":{"build":"vite build"}}"#;
    let fixture = git_fixture(&[("package.json", package), ("index.html", "<html></html>")]).await;
    let request = |branch: &str, root_dir: &str| SnapshotRequest {
        repo_url: fixture.url.clone(),
        branch: branch.to_string(),
        root_dir: root_dir.to_string(),
        token: Some("secret-token".to_string()),
    };

    let snapshot = git::snapshot_with(&request("main", ""), RepoPolicy::Unrestricted)
        .await
        .unwrap();
    assert!(snapshot.files.contains("package.json"));
    assert!(
        git::snapshot_with(&request("main", "nowhere"), RepoPolicy::Unrestricted)
            .await
            .is_err()
    );
    let missing_branch = git::snapshot_with(&request("ghost", ""), RepoPolicy::Unrestricted).await;
    assert_eq!(
        missing_branch.err().map(|error| error.to_string()),
        Some("git error: clone failed".to_string())
    );

    let route = json!({ "repo_url": fixture.url, "branch": "main" });
    let (status, _) = call(
        &harness.app,
        Method::POST,
        "/internal/detect",
        Some(SECRET),
        Some(route),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
