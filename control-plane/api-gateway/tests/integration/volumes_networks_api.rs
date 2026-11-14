use axum::http::{Method, StatusCode};
use serde_json::Value;

use crate::common::harness;
use crate::support_a::raw;

async fn post_text(uri: &str, body: &str) -> (StatusCode, Value) {
    let harness = harness().await;
    let (status, _, bytes) = raw(
        &harness,
        Method::POST,
        uri,
        &[
            ("authorization", &format!("Bearer {}", harness.token)),
            ("content-type", "text/plain"),
            ("x-request-marker", "marker"),
        ],
        body.as_bytes().to_vec(),
    )
    .await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn volume_bodies_that_are_not_json_are_forwarded_empty() {
    for suffix in [
        "",
        "/00000000-0000-0000-0000-0000000000a1/attach",
        "/00000000-0000-0000-0000-0000000000a1/snapshots",
    ] {
        let (status, echoed) = post_text(&format!("/api/volumes{suffix}"), "not json").await;
        assert_eq!(status, StatusCode::OK, "{suffix}");
        assert!(echoed["body"].is_null(), "{suffix}");
        assert_eq!(echoed["path"], format!("/volumes{suffix}"));
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn network_endpoints_reject_non_json_bodies() {
    for suffix in ["", "/net-1/policies", "/net-1/members"] {
        let (status, _) = post_text(&format!("/api/networks{suffix}"), "not json").await;
        assert!(status.is_client_error(), "{suffix}: {status}");
    }
}
