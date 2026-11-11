use axum::http::{Method, StatusCode};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::{json, Value};

use crate::common::Harness;
use crate::support_b::{full_harness, raw, SETUP_CODE};

async fn remove_marker(harness: &Harness) -> Option<entity::entities::system_settings::Model> {
    let marker = entity::SystemSettings::find_by_id("setup.completed")
        .one(&harness.db)
        .await
        .unwrap();
    entity::SystemSettings::delete_by_id("setup.completed")
        .exec(&harness.db)
        .await
        .unwrap();
    marker
}

async fn restore_marker(
    harness: &Harness,
    marker: Option<entity::entities::system_settings::Model>,
) {
    let Some(marker) = marker else {
        return;
    };
    entity::SystemSettings::delete_by_id("setup.completed")
        .exec(&harness.db)
        .await
        .unwrap();
    entity::entities::system_settings::ActiveModel {
        key: Set(marker.key),
        value: Set(marker.value),
        updated_at: Set(marker.updated_at),
    }
    .insert(&harness.db)
    .await
    .unwrap();
}

async fn submit(
    harness: &Harness,
    code: &str,
    password: &str,
    pin: &str,
    organization: &str,
) -> (StatusCode, Value) {
    let body = json!({
        "setup_code": code,
        "organization_name": organization,
        "admin_email": "owner@csfx.test",
        "encrypted_password": harness.encrypted(password).await,
        "encrypted_console_pin": harness.encrypted(pin).await,
    });
    let response = raw(harness, Method::POST, "/api/setup", &[], Some(body)).await;
    (response.status, response.body)
}

async fn join(harness: &Harness, code: &str, join_string: &str) -> StatusCode {
    let body = json!({"setup_code": code, "join_string": join_string});
    raw(harness, Method::POST, "/api/setup/join", &[], Some(body))
        .await
        .status
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn setup_status_and_ca_download_are_public() {
    let harness = full_harness().await;
    let response = raw(&harness, Method::GET, "/api/setup/status", &[], None).await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(["ready", "setup_required"].contains(&response.body["state"].as_str().unwrap()));
    assert!(response.body["ca_fingerprint"]
        .as_str()
        .unwrap()
        .contains(':'));

    let ca = raw(&harness, Method::GET, "/api/setup/ca.crt", &[], None).await;
    assert_eq!(ca.status, StatusCode::OK);
    assert_eq!(ca.headers["content-type"], "application/x-pem-file");
    assert!(ca.headers["content-disposition"]
        .to_str()
        .unwrap()
        .contains("csfx-ca.crt"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn setup_validates_code_and_input_before_completing() {
    let harness = full_harness().await;
    let completed = entity::SystemSettings::find_by_id("setup.completed")
        .one(&harness.db)
        .await
        .unwrap();
    if completed.is_some() {
        let (status, _) = submit(
            &harness,
            SETUP_CODE,
            "long-enough-password",
            "123456",
            "Org",
        )
        .await;
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(join(&harness, SETUP_CODE, "x").await, StatusCode::GONE);
    }
    let marker = remove_marker(&harness).await;

    let state = raw(&harness, Method::GET, "/api/setup/status", &[], None)
        .await
        .body["state"]
        .clone();
    let wrong = submit(
        &harness,
        "WRONG-CODE",
        "long-enough-password",
        "123456",
        "Org",
    )
    .await;
    let blank = submit(&harness, SETUP_CODE, "long-enough-password", "123456", "  ").await;
    let short = submit(&harness, SETUP_CODE, "short", "123456", "Org").await;
    let undecryptable = json!({
        "setup_code": SETUP_CODE, "organization_name": "Org", "admin_email": "a@b.co",
        "encrypted_password": "not-base64", "encrypted_console_pin": "x",
    });
    let broken = raw(
        &harness,
        Method::POST,
        "/api/setup",
        &[],
        Some(undecryptable),
    )
    .await;

    let wrong_join = join(&harness, "WRONG", "x").await;
    let bad_join = join(&harness, SETUP_CODE, "not a join string").await;
    let good_join = join(
        &harness,
        SETUP_CODE,
        "csfx-join.10.0.0.5:8000#csfx-bootstrap.ab12#cd34",
    )
    .await;
    restore_marker(&harness, marker).await;

    assert_eq!(state, "setup_required");
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED);
    assert_eq!(blank.0, StatusCode::BAD_REQUEST);
    assert_eq!(short.0, StatusCode::BAD_REQUEST);
    assert_eq!(broken.status, StatusCode::BAD_REQUEST);
    assert_eq!(wrong_join, StatusCode::UNAUTHORIZED);
    assert_eq!(bad_join, StatusCode::BAD_REQUEST);
    assert_eq!(good_join, StatusCode::ACCEPTED);
    let queued = std::fs::read_to_string(std::env::var("CSFX_JOIN_REQUEST_FILE").unwrap()).unwrap();
    assert!(queued.starts_with("csfx-join."));
}
