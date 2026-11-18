use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};

use crate::common::{harness, Harness};

const PASSPHRASE: &str = "correct horse battery";

async fn put_target(harness: &Harness, name: &str, body: Value) -> Value {
    let (status, view) = harness
        .admin(
            Method::PUT,
            &format!("/api/admin/export-targets/{name}"),
            Some(body),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    view
}

fn local(path: &Path, passphrase: Option<&str>) -> Value {
    json!({ "kind": "local", "path": path, "passphrase": passphrase, "default": true })
}

async fn export(harness: &Harness, scope: Value, target: Option<&str>) -> (String, Value) {
    let (status, created) = harness
        .admin(
            Method::POST,
            "/api/exports",
            Some(json!({ "scope": scope, "target": target })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    let id = created["id"].as_str().unwrap().to_string();
    for _ in 0..300 {
        let (_, row) = harness.get(&format!("/api/exports/{id}")).await;
        if row["status"] != "running" {
            return (id, row);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("export did not finish");
}

async fn restore(
    harness: &Harness,
    id: &str,
    target: Option<&str>,
    pass: Option<&str>,
) -> StatusCode {
    let (status, _) = harness
        .admin(
            Method::POST,
            &format!("/api/exports/{id}/restore"),
            Some(json!({ "target": target, "passphrase": pass })),
        )
        .await;
    status
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn config_export_round_trips_and_detects_tampering() {
    let harness = harness().await;
    let root = std::env::temp_dir().join(format!("csfx-export-{}", uuid::Uuid::new_v4()));
    let config = json!({ "kind": "config" });

    put_target(&harness, "encrypted", local(&root, Some(PASSPHRASE))).await;
    let (encrypted_id, encrypted) = export(&harness, config.clone(), Some("encrypted")).await;
    assert_eq!(encrypted["status"], "completed", "{encrypted}");
    assert!(encrypted["manifest"]["encryption"].is_object());
    assert_eq!(encrypted["manifest"]["scope"]["kind"], "config");
    let wrong = restore(
        &harness,
        &encrypted_id,
        Some("encrypted"),
        Some("wrong passphrase"),
    )
    .await;
    assert_eq!(wrong, StatusCode::BAD_REQUEST);
    let right = restore(&harness, &encrypted_id, Some("encrypted"), None).await;
    assert_eq!(right, StatusCode::OK);

    put_target(&harness, "plain", local(&root, None)).await;
    let (plain_id, plain) = export(&harness, config, Some("plain")).await;
    assert!(plain["manifest"]["encryption"].is_null());
    assert_eq!(
        restore(&harness, &plain_id, Some("plain"), None).await,
        StatusCode::OK
    );

    let (status, roles) = harness
        .get(&format!(
            "/api/exports/{plain_id}/artifacts/config/role.json?target=plain"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(roles.as_array().is_some_and(|rows| !rows.is_empty()));
    let (status, _) = harness
        .get(&format!(
            "/api/exports/{plain_id}/artifacts/config/missing.json?target=plain"
        ))
        .await;
    assert_ne!(status, StatusCode::OK);

    std::fs::write(root.join(&plain_id).join("config/role.json"), b"[]").unwrap();
    assert_eq!(
        restore(&harness, &plain_id, Some("plain"), None).await,
        StatusCode::CONFLICT
    );

    let (status, _) = harness
        .admin(Method::POST, "/api/admin/export-targets/plain/test", None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = harness
        .admin(Method::DELETE, "/api/admin/export-targets/plain", None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL and an S3 endpoint in CSFX_TEST_S3_*"]
async fn config_export_round_trips_through_s3() {
    let harness = harness().await;
    let var = |name: &str| std::env::var(name).expect(name);
    let body = json!({
        "kind": "s3",
        "endpoint": var("CSFX_TEST_S3_ENDPOINT"),
        "region": "us-east-1",
        "bucket": var("CSFX_TEST_S3_BUCKET"),
        "prefix": "csfx/test",
        "access_key_id": var("CSFX_TEST_S3_KEY"),
        "secret_access_key": var("CSFX_TEST_S3_SECRET"),
        "passphrase": PASSPHRASE,
        "default": true,
    });
    put_target(&harness, "remote", body).await;
    let (status, _) = harness
        .admin(Method::POST, "/api/admin/export-targets/remote/test", None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (id, row) = export(&harness, json!({ "kind": "config" }), Some("remote")).await;
    assert_eq!(row["status"], "completed", "{row}");
    assert_eq!(
        restore(&harness, &id, Some("remote"), None).await,
        StatusCode::OK
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn exports_enforce_scope_permissions_and_validation() {
    let harness = harness().await;
    let token = harness.viewer_token().await;
    let (status, _) = harness
        .send(Method::GET, "/api/exports", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let body = json!({ "scope": { "kind": "config" } });
    let (status, _) = harness
        .send(Method::POST, "/api/exports", Some(&token), Some(body))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let missing = json!({ "scope": { "kind": "volume", "id": uuid::Uuid::new_v4() } });
    let (status, _) = harness
        .admin(Method::POST, "/api/exports", Some(missing))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let no_id = json!({ "scope": { "kind": "volume" } });
    let (status, _) = harness
        .admin(Method::POST, "/api/exports", Some(no_id))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
