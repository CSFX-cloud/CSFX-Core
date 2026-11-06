use axum::http::{Method, StatusCode};
use serde_json::json;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::common::{harness, ADMIN_PASSWORD};

fn current_code(secret: &str, username: &str) -> String {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        Secret::Encoded(secret.to_string()).to_bytes().unwrap(),
        Some("CSFX-Core".to_string()),
        username.to_string(),
    )
    .unwrap()
    .generate_current()
    .unwrap()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn public_key_is_served_without_authentication() {
    let harness = harness().await;

    let (status, body) = harness
        .send(Method::GET, "/api/public-key", None, None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["public_key"]
        .as_str()
        .unwrap()
        .contains("RSA PUBLIC KEY"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn profile_requires_a_valid_token() {
    let harness = harness().await;

    let (status, _) = harness.send(Method::GET, "/api/profile", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = harness
        .send(Method::GET, "/api/profile", Some("garbage"), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, profile) = harness.get("/api/profile").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(profile["username"], harness.admin_name);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn login_issues_a_session_token() {
    let harness = harness().await;
    let encrypted = harness.encrypted(ADMIN_PASSWORD).await;

    let (status, body) = harness
        .send(
            Method::POST,
            "/api/login",
            None,
            Some(json!({"username": harness.admin_name, "encrypted_password": encrypted})),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], harness.admin_name);
    let token = body["token"].as_str().unwrap();
    let (status, _) = harness
        .send(Method::GET, "/api/validate-session", Some(token), None)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn login_rejects_bad_credentials() {
    let harness = harness().await;
    let wrong = harness.encrypted("not-the-password").await;
    let right = harness.encrypted(ADMIN_PASSWORD).await;

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/login",
            None,
            Some(json!({"username": harness.admin_name, "encrypted_password": wrong})),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/login",
            None,
            Some(json!({"username": "nobody", "encrypted_password": right})),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/login",
            None,
            Some(json!({"username": harness.admin_name, "encrypted_password": "###"})),
        )
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn two_factor_lifecycle_gates_login() {
    let harness = harness().await;
    let (_, name, token) = harness.user_with_token().await;
    let call = |method, uri: &'static str, body| {
        harness.send(method, uri, Some(token.as_str()), Some(body))
    };

    let (status, setup) = harness
        .send(Method::POST, "/api/2fa/setup", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let secret = setup["secret"].as_str().unwrap().to_string();
    assert!(setup["qr_code"].is_string());

    let (status, _) = call(Method::POST, "/api/2fa/enable", json!({"code": "000000"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        Method::POST,
        "/api/2fa/enable",
        json!({"code": current_code(&secret, &name)}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let encrypted = harness.encrypted(ADMIN_PASSWORD).await;
    let login = |code: Option<String>| {
        harness.send(
            Method::POST,
            "/api/login",
            None,
            Some(json!({
                "username": name,
                "encrypted_password": encrypted,
                "two_factor_code": code
            })),
        )
    };
    assert_eq!(login(None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        login(Some("000000".to_string())).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(Some(current_code(&secret, &name))).await.0,
        StatusCode::OK
    );

    let (status, _) = call(Method::POST, "/api/2fa/disable", json!({"code": "000000"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        Method::POST,
        "/api/2fa/disable",
        json!({"code": current_code(&secret, &name)}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(login(None).await.0, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn enabling_two_factor_without_setup_is_rejected() {
    let harness = harness().await;
    let (_, _, token) = harness.user_with_token().await;

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/2fa/enable",
            Some(&token),
            Some(json!({"code": "123456"})),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn password_change_replaces_credentials() {
    let harness = harness().await;
    let (_, name, token) = harness.user_with_token().await;
    let old = harness.encrypted(ADMIN_PASSWORD).await;
    let new = harness.encrypted("a-brand-new-secret").await;

    let wrong = harness.encrypted("wrong-old-password").await;
    let (status, _) = harness
        .send(
            Method::POST,
            "/api/change-password",
            Some(&token),
            Some(json!({"old_password": wrong, "new_password": new})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/change-password",
            Some(&token),
            Some(json!({"old_password": old, "new_password": new})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let login = |encrypted: String| {
        harness.send(
            Method::POST,
            "/api/login",
            None,
            Some(json!({"username": name, "encrypted_password": encrypted})),
        )
    };
    assert_eq!(login(old).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(login(new).await.0, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn email_and_gravatar_updates_are_persisted() {
    let harness = harness().await;
    let (_, _, token) = harness.user_with_token().await;
    let address = format!("{}@csfx.test", uuid::Uuid::new_v4().simple());

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/change-email",
            Some(&token),
            Some(json!({"new_email": address})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = harness
        .send(
            Method::POST,
            "/api/change-email",
            Some(&token),
            Some(json!({"new_email": address})),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/change-gravatar-email",
            Some(&token),
            Some(json!({"gravatar_email": "avatar@csfx.test"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, profile) = harness
        .send(Method::GET, "/api/profile", Some(&token), None)
        .await;
    assert_eq!(profile["email"], address);
    assert_eq!(profile["gravatar_email"], "avatar@csfx.test");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn logout_succeeds_for_authenticated_user() {
    let harness = harness().await;
    let (_, _, token) = harness.user_with_token().await;

    let (status, body) = harness
        .send(Method::POST, "/api/logout", Some(&token), None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "Logged out successfully");
}
