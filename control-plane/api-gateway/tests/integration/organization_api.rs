use axum::http::StatusCode;
use serde_json::json;

use crate::common::harness;

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn organization_can_be_read_and_updated() {
    let harness = harness().await;

    let (status, organization) = harness.get("/api/organization").await;
    assert_eq!(status, StatusCode::OK);
    assert!(organization["name"].is_string());

    let (status, updated) = harness
        .admin(
            axum::http::Method::PUT,
            "/api/organization",
            Some(json!({"name": organization["name"], "description": "updated by test"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["description"], "updated by test");

    let (status, roles) = harness.get("/api/organization/roles").await;
    assert_eq!(status, StatusCode::OK);
    assert!(roles
        .as_array()
        .unwrap()
        .iter()
        .any(|role| role["name"] == "Admin"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn limited_members_cannot_manage_the_organization() {
    let harness = harness().await;
    let limited = harness.limited_token().await;

    let (status, _) = harness
        .send(
            axum::http::Method::PUT,
            "/api/organization",
            Some(&limited),
            Some(json!({"name": "x", "description": null})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    for uri in [
        "/api/organization/users",
        &format!("/api/organization/users/{}", harness.admin_id),
    ] {
        let (status, _) = harness
            .send(axum::http::Method::GET, uri, Some(&limited), None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    let (status, _) = harness
        .send(
            axum::http::Method::POST,
            "/api/organization/users",
            Some(&limited),
            Some(json!({
                "username": "x",
                "email": null,
                "password": "x",
                "role_id": uuid::Uuid::new_v4(),
                "force_password_change": false
            })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn user_management_lifecycle() {
    let harness = harness().await;
    let (_, roles) = harness.get("/api/organization/roles").await;
    let role_id = roles.as_array().unwrap()[0]["id"].clone();
    let username = format!("managed-{}", uuid::Uuid::new_v4().simple());

    let (status, created) = harness
        .post(
            "/api/organization/users",
            json!({
                "username": username,
                "email": format!("{}@csfx.test", username),
                "password": "initial-password",
                "role_id": role_id,
                "force_password_change": true
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["force_password_change"], true);
    assert_eq!(created["auth_source"], "local");

    let (status, fetched) = harness
        .get(&format!("/api/organization/users/{}", id))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["username"], username);

    let (_, listed) = harness.get("/api/organization/users").await;
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|user| user["id"] == id));

    let (status, updated) = harness
        .admin(
            axum::http::Method::PUT,
            &format!("/api/organization/users/{}", id),
            Some(json!({"email": "changed@csfx.test", "force_password_change": false})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["email"], "changed@csfx.test");
    assert_eq!(updated["force_password_change"], false);

    let (status, _) = harness
        .admin(
            axum::http::Method::PUT,
            &format!("/api/organization/users/{}/role", id),
            Some(json!({"role_id": role_id})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = harness
        .admin(
            axum::http::Method::DELETE,
            &format!("/api/organization/users/{}", id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = harness
        .get(&format!("/api/organization/users/{}", id))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_users_are_not_found() {
    let harness = harness().await;
    let uri = format!("/api/organization/users/{}", uuid::Uuid::new_v4());

    let (status, _) = harness.get(&uri).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = harness
        .admin(
            axum::http::Method::PUT,
            &uri,
            Some(json!({"email": null, "force_password_change": null})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
