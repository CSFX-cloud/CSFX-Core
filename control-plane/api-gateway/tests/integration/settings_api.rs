use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::common::harness;

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn logs_retention_round_trips_within_bounds() {
    let harness = harness().await;
    let uri = "/api/admin/settings/logs-retention";

    for days in [14, 30] {
        let (status, body) = harness
            .admin(Method::PUT, uri, Some(json!({"retention_days": days})))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["retention_days"], days);
        let (_, current) = harness.get(uri).await;
        assert_eq!(current["retention_days"], days);
    }

    for days in [0, 366] {
        let (status, _) = harness
            .admin(Method::PUT, uri, Some(json!({"retention_days": days})))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn avatar_fallback_accepts_known_styles_only() {
    let harness = harness().await;
    let uri = "/api/settings/avatar-fallback";

    for style in ["blobatar", "initials"] {
        let (status, body) = harness
            .admin(Method::PUT, uri, Some(json!({"style": style})))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["style"], style);
        let (_, current) = harness.get(uri).await;
        assert_eq!(current["style"], style);
    }

    let (status, _) = harness
        .admin(Method::PUT, uri, Some(json!({"style": "pixelated"})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn notification_settings_validate_and_persist() {
    let harness = harness().await;
    let uri = "/api/admin/settings/notifications";

    let (status, initial) = harness.get(uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(initial["enabled"].is_boolean());

    let (status, body) = harness
        .admin(
            Method::PUT,
            uri,
            Some(json!({
                "enabled": false,
                "server_url": "https://ntfy.csfx.test",
                "topic": "alerts",
                "token": "secret-token",
                "min_severity": "warning"
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["topic"], "alerts");
    assert_eq!(body["token_set"], true);
    assert!(body.get("token").is_none());

    let (status, _) = harness
        .admin(
            Method::PUT,
            uri,
            Some(json!({
                "enabled": true,
                "server_url": "not a url",
                "topic": "",
                "token": null,
                "min_severity": "bogus"
            })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn settings_require_permissions() {
    let harness = harness().await;
    let limited = harness.limited_token().await;

    for (method, uri) in [
        (Method::GET, "/api/admin/settings/logs-retention"),
        (Method::GET, "/api/admin/settings/notifications"),
        (Method::PUT, "/api/settings/avatar-fallback"),
    ] {
        let (status, _) = harness
            .send(
                method,
                uri,
                Some(&limited),
                Some(json!({"style": "initials"})),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{}", uri);
    }
}
