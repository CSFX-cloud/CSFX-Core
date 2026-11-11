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

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn role_changes_cannot_escalate_privileges() {
    use axum::http::Method;

    let harness = harness().await;
    let (actor_id, actor) = harness
        .token_with_permissions(&["users.update", "users.create", "agents.view"])
        .await;
    let (target_id, _, _) = harness.user_with_token().await;
    let admin_role = harness.admin_role().await;
    let viewer_role = harness.role_with_permissions(&["agents.view"]).await;
    let role_uri = |id| format!("/api/organization/users/{}/role", id);

    let (status, _) = harness
        .send(
            Method::PUT,
            &role_uri(actor_id),
            Some(&actor),
            Some(json!({"role_id": admin_role})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = harness
        .send(
            Method::PUT,
            &role_uri(target_id),
            Some(&actor),
            Some(json!({"role_id": viewer_role})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (limited_id, _) = harness.token_with_permissions(&[]).await;
    let (status, _) = harness
        .send(
            Method::PUT,
            &role_uri(limited_id),
            Some(&actor),
            Some(json!({"role_id": admin_role})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = harness
        .send(
            Method::PUT,
            &role_uri(limited_id),
            Some(&actor),
            Some(json!({"role_id": viewer_role})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn users_cannot_be_created_with_unheld_or_foreign_roles() {
    use axum::http::Method;

    let harness = harness().await;
    let (_, actor) = harness
        .token_with_permissions(&["users.create", "agents.view"])
        .await;
    let admin_role = harness.admin_role().await;
    let viewer_role = harness.role_with_permissions(&["agents.view"]).await;
    let create = |role_id: uuid::Uuid, name: &str| {
        json!({
            "username": name,
            "email": null,
            "password": "Passw0rd!Passw0rd",
            "role_id": role_id,
            "force_password_change": false
        })
    };

    for (role_id, name, expected) in [
        (admin_role, "escalated", StatusCode::FORBIDDEN),
        (uuid::Uuid::new_v4(), "foreign", StatusCode::NOT_FOUND),
    ] {
        let (status, _) = harness
            .send(
                Method::POST,
                "/api/organization/users",
                Some(&actor),
                Some(create(role_id, name)),
            )
            .await;
        assert_eq!(status, expected, "{name}");
    }

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/organization/users",
            Some(&actor),
            Some(create(viewer_role, "delegated")),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn roles_cannot_carry_permissions_the_author_lacks() {
    use axum::http::Method;

    let harness = harness().await;
    let (_, actor) = harness
        .token_with_permissions(&["roles.manage", "agents.view"])
        .await;
    let held = harness.permission_id("agents.view").await;
    let unheld = harness.permission_id("users.delete").await;

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/roles",
            Some(&actor),
            Some(json!({"name": "greedy", "description": null, "permission_ids": [held, unheld]})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = harness
        .send(
            Method::POST,
            "/api/roles",
            Some(&actor),
            Some(json!({"name": "modest", "description": null, "permission_ids": [held]})),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn the_last_admin_of_an_organization_is_protected() {
    use api_gateway::role_grants::{ensure_not_last_admin, GrantError};
    use sea_orm::{ActiveModelTrait, Set};

    let harness = harness().await;
    let now = chrono::Utc::now().naive_utc();
    let org_id = uuid::Uuid::new_v4();
    entity::organization::ActiveModel {
        id: Set(org_id),
        name: Set(format!("org-{}", org_id.simple())),
        description: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&harness.db)
    .await
    .unwrap();
    let admin_role = crate::common::create_role(&harness.db, org_id).await;
    entity::role::ActiveModel {
        id: Set(admin_role),
        name: Set("Admin".to_string()),
        description: Set(None),
        organization_id: Set(org_id),
        is_system_role: Set(true),
        created_at: Set(now),
    }
    .update(&harness.db)
    .await
    .unwrap();

    let (first, _) = crate::common::create_user(&harness.db, org_id, admin_role).await;
    let result = ensure_not_last_admin(&harness.db, org_id, first).await;
    assert!(matches!(result, Err(GrantError::LastAdmin)));

    let (second, _) = crate::common::create_user(&harness.db, org_id, admin_role).await;
    assert!(ensure_not_last_admin(&harness.db, org_id, first)
        .await
        .is_ok());
    assert!(ensure_not_last_admin(&harness.db, org_id, second)
        .await
        .is_ok());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn user_details_cannot_be_changed_for_more_privileged_users() {
    use axum::http::Method;

    let harness = harness().await;
    let (_, actor) = harness.token_with_permissions(&["users.update"]).await;
    let (admin_id, _, _) = harness.user_with_token().await;
    let (peer_id, _) = harness.token_with_permissions(&[]).await;
    let uri = |id| format!("/api/organization/users/{}", id);
    let email = format!("{}@csfx.test", uuid::Uuid::new_v4().simple());

    let (status, _) = harness
        .send(
            Method::PUT,
            &uri(admin_id),
            Some(&actor),
            Some(json!({"force_password_change": true})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = harness
        .send(
            Method::PUT,
            &uri(peer_id),
            Some(&actor),
            Some(json!({"email": email})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (other_id, _) = harness.token_with_permissions(&[]).await;
    let (status, _) = harness
        .send(
            Method::PUT,
            &uri(other_id),
            Some(&actor),
            Some(json!({"email": email})),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
