use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::common::{harness, Harness};

async fn permission_ids(harness: &Harness, count: usize) -> Vec<Value> {
    let (status, permissions) = harness.get("/api/permissions").await;
    assert_eq!(status, StatusCode::OK);
    permissions
        .as_array()
        .unwrap()
        .iter()
        .take(count)
        .map(|permission| permission["id"].clone())
        .collect()
}

async fn create_role(harness: &Harness, name: &str, permissions: Vec<Value>) -> Value {
    let (status, role) = harness
        .post(
            "/api/roles",
            json!({"name": name, "description": "test role", "permission_ids": permissions}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    role
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn role_endpoints_require_the_manage_permission() {
    let harness = harness().await;
    let limited = harness.limited_token().await;

    for (method, uri) in [
        (Method::GET, "/api/permissions"),
        (Method::GET, "/api/roles"),
    ] {
        let (status, _) = harness
            .send(method.clone(), uri, Some(&limited), None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = harness.send(method, uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn role_lifecycle_creates_updates_and_deletes() {
    let harness = harness().await;
    let name = format!("custom-{}", uuid::Uuid::new_v4().simple());
    let permissions = permission_ids(&harness, 3).await;

    let created = create_role(&harness, &name, permissions[..2].to_vec()).await;
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["is_system_role"], false);
    assert_eq!(created["permission_ids"].as_array().unwrap().len(), 2);

    let (_, roles) = harness.get("/api/roles").await;
    let listed = roles
        .as_array()
        .unwrap()
        .iter()
        .find(|role| role["id"] == id)
        .unwrap();
    assert_eq!(listed["permission_ids"].as_array().unwrap().len(), 2);

    let renamed = format!("{}-renamed", name);
    let (status, updated) = harness
        .admin(
            Method::PUT,
            &format!("/api/roles/{}", id),
            Some(json!({"name": renamed, "description": null, "permission_ids": [permissions[2]]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], renamed);
    assert_eq!(updated["permission_ids"].as_array().unwrap().len(), 1);

    let (status, _) = harness
        .admin(Method::DELETE, &format!("/api/roles/{}", id), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = harness
        .admin(Method::DELETE, &format!("/api/roles/{}", id), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn role_creation_validates_input() {
    let harness = harness().await;
    let name = format!("dup-{}", uuid::Uuid::new_v4().simple());
    create_role(&harness, &name, vec![]).await;

    let (status, _) = harness
        .post(
            "/api/roles",
            json!({"name": name, "description": null, "permission_ids": []}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = harness
        .post(
            "/api/roles",
            json!({"name": "   ", "description": null, "permission_ids": []}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = harness
        .post(
            "/api/roles",
            json!({
                "name": format!("ghost-{}", uuid::Uuid::new_v4().simple()),
                "description": null,
                "permission_ids": [uuid::Uuid::new_v4()]
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn system_and_assigned_roles_are_protected() {
    let harness = harness().await;
    let (_, roles) = harness.get("/api/roles").await;
    let system = roles
        .as_array()
        .unwrap()
        .iter()
        .find(|role| role["is_system_role"] == true)
        .unwrap();
    let system_uri = format!("/api/roles/{}", system["id"].as_str().unwrap());

    let (status, _) = harness
        .admin(
            Method::PUT,
            &system_uri,
            Some(json!({"name": "hacked", "description": null, "permission_ids": []})),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = harness.admin(Method::DELETE, &system_uri, None).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let name = format!("busy-{}", uuid::Uuid::new_v4().simple());
    let busy = create_role(&harness, &name, vec![]).await;
    let busy_id: uuid::Uuid = busy["id"].as_str().unwrap().parse().unwrap();
    crate::common::create_user(&harness.db, harness.org_id, busy_id).await;
    let (status, _) = harness
        .admin(Method::DELETE, &format!("/api/roles/{}", busy_id), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
