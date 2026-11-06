use axum::http::{Method, StatusCode};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde_json::{json, Value};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

use crate::common::Harness;
use crate::support_b::{full_harness, resource_group};

const TOTP_SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

fn base(group: Uuid) -> String {
    format!("/api/resource-groups/{group}/vaults")
}

async fn new_vault(harness: &Harness) -> (Uuid, Uuid, String) {
    let group = resource_group(harness).await;
    let name = format!("v-{}", Uuid::new_v4().simple());
    let (status, body) = harness.post(&base(group), json!({"name": name})).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body["id"].as_str().unwrap().parse().unwrap();
    (group, id, format!("{}/{id}", base(group)))
}

async fn put_secret(harness: &Harness, vault: &str, name: &str, value: &str) -> Value {
    let body = json!({"name": name, "value": value, "expires_at": "2030-01-01"});
    let (status, created) = harness.post(&format!("{vault}/secrets"), body).await;
    assert_eq!(status, StatusCode::CREATED);
    created
}

async fn enable_two_factor(harness: &Harness) -> String {
    let user = entity::User::find_by_id(harness.admin_id)
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: entity::user::ActiveModel = user.into();
    active.two_factor_enabled = Set(true);
    active.two_factor_secret = Set(Some(TOTP_SECRET.to_string()));
    active.update(&harness.db).await.unwrap();
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        Secret::Encoded(TOTP_SECRET.to_string()).to_bytes().unwrap(),
        None,
        harness.admin_name.clone(),
    )
    .unwrap();
    totp.generate_current().unwrap()
}

async fn audit_actions(harness: &Harness, vault: &str) -> Vec<String> {
    let (_, entries) = harness.get(&format!("{vault}/audit")).await;
    entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vault_lifecycle_and_validation() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let uri = base(group);

    for name in ["", "UPPER", "has space"] {
        let (status, _) = harness.post(&uri, json!({"name": name})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name}");
    }
    let missing = base(Uuid::new_v4());
    assert_eq!(
        harness.post(&missing, json!({"name": "ok"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(harness.get(&missing).await.0, StatusCode::NOT_FOUND);

    let (other_group, _, vault) = new_vault(&harness).await;
    let (status, listed) = harness.get(&base(group)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 0);

    assert_eq!(harness.get(&vault).await.0, StatusCode::OK);
    let crossed = vault.replace(&other_group.to_string(), &group.to_string());
    assert_eq!(harness.get(&crossed).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vault_overview_seal_and_delete() {
    let harness = full_harness().await;
    let (group, id, vault) = new_vault(&harness).await;
    put_secret(&harness, &vault, "TOKEN", "abc").await;

    let (status, overview) = harness.get(&vault).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["secret_count"], 1);
    assert_eq!(overview["sealed"], false);

    let (status, sealed) = harness.post(&format!("{vault}/seal"), json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sealed["sealed"], true);
    let (status, _) = harness.get(&format!("{vault}/secrets")).await;
    assert_eq!(status, StatusCode::LOCKED);
    let blocked = json!({"name": "OTHER", "value": "x"});
    let (status, _) = harness.post(&format!("{vault}/secrets"), blocked).await;
    assert_eq!(status, StatusCode::LOCKED);

    let (status, open) = harness.post(&format!("{vault}/unseal"), json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(open["sealed"], false);

    let (status, listed) = harness.get(&base(group)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 1);

    let (status, _) = harness.admin(Method::DELETE, &vault, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(harness.get(&vault).await.0, StatusCode::NOT_FOUND);
    let remaining = entity::Vaults::find_by_id(id)
        .one(&harness.db)
        .await
        .unwrap();
    assert!(remaining.is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn secret_crud_versions_and_restore() {
    let harness = full_harness().await;
    let (_, _, vault) = new_vault(&harness).await;

    let created = put_secret(&harness, &vault, "API_KEY", "one").await;
    assert_eq!(created["version"], 1);
    let uri = format!("{vault}/secrets/API_KEY");
    let (status, updated) = harness
        .admin(Method::PUT, &uri, Some(json!({"value": "two"})))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["version"], 2);

    let (status, listed) = harness.get(&format!("{vault}/secrets")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["name"], "API_KEY");
    assert_eq!(listed[0]["version"], 2);

    let (status, versions) = harness.get(&format!("{uri}/versions")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(versions[0]["version"], 2);
    assert_eq!(versions.as_array().unwrap().len(), 2);

    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    let (_, listed) = harness.get(&format!("{vault}/secrets")).await;
    assert_eq!(listed[0]["deleted"], true);
    let (status, _) = harness
        .post(&format!("{uri}/restore"), json!({"version": 2}))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, listed) = harness.get(&format!("{vault}/secrets")).await;
    assert_eq!(listed[0]["deleted"], false);

    let actions = audit_actions(&harness, &vault).await;
    assert!(actions
        .iter()
        .any(|action| action.starts_with("restore API_KEY")));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn secret_validation_and_store_errors() {
    let harness = full_harness().await;
    let (_, _, vault) = new_vault(&harness).await;
    let secrets = format!("{vault}/secrets");

    for body in [
        json!({"value": "x"}),
        json!({"name": "bad/name", "value": "x"}),
        json!({"name": "OK", "value": ""}),
    ] {
        assert_eq!(
            harness.post(&secrets, body).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let boom = json!({"name": "boom", "value": "x"});
    assert_eq!(
        harness.post(&secrets, boom).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let reject = json!({"name": "reject", "value": "x"});
    assert_eq!(
        harness.post(&secrets, reject).await.0,
        StatusCode::BAD_GATEWAY
    );

    let bad = format!("{secrets}/bad.name");
    assert_eq!(
        harness.admin(Method::DELETE, &bad, None).await.0,
        StatusCode::BAD_REQUEST
    );
    let missing = format!("{secrets}/NOPE/versions");
    assert_eq!(harness.get(&missing).await.0, StatusCode::NOT_FOUND);
    let restore = format!("{secrets}/NOPE/restore");
    assert_eq!(
        harness.post(&restore, json!({"version": 1})).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn grants_validate_persist_and_revoke() {
    let harness = full_harness().await;
    let (_, _, vault) = new_vault(&harness).await;
    let grants = format!("{vault}/grants");

    for body in [
        json!({"principal": "app", "capability": "nope"}),
        json!({"principal": "bad/app", "capability": "read_secrets"}),
        json!({"principal": "app", "capability": "read_secrets", "scope": "a/b"}),
        json!({"principal": "", "capability": "reveal_secrets"}),
    ] {
        assert_eq!(harness.post(&grants, body).await.0, StatusCode::BAD_REQUEST);
    }

    let body = json!({"principal": "web.app", "capability": "read_secrets", "scope": "DB_*"});
    let (status, grant) = harness.post(&grants, body).await;
    assert_eq!(status, StatusCode::CREATED);
    let reveal = json!({"principal": "someone@csfx.test", "capability": "reveal_secrets"});
    assert_eq!(harness.post(&grants, reveal).await.0, StatusCode::CREATED);

    let (_, listed) = harness.get(&grants).await;
    assert_eq!(listed.as_array().unwrap().len(), 2);

    let issued = harness
        .post(&format!("{vault}/principals/web.app/secret-id"), json!({}))
        .await;
    assert_eq!(issued.0, StatusCode::OK);
    assert_eq!(issued.1["role_id"], "mock-role-id");
    assert_eq!(issued.1["wrapping_token"], "mock-wrap");
    let unknown = format!("{vault}/principals/ghost/secret-id");
    assert_eq!(
        harness.post(&unknown, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    let invalid = format!("{vault}/principals/bad%2Fname/secret-id");
    assert_ne!(harness.post(&invalid, json!({})).await.0, StatusCode::OK);

    let target = format!("{grants}/{}", grant["id"].as_str().unwrap());
    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, listed) = harness.get(&grants).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn reveal_enforces_two_factor_and_validation() {
    let harness = full_harness().await;
    let (_, _, vault) = new_vault(&harness).await;
    put_secret(&harness, &vault, "TOKEN", "s3cret").await;
    let uri = format!("{vault}/secrets/TOKEN/reveal");

    let blank = json!({"reason": "  ", "code": "000000"});
    assert_eq!(harness.post(&uri, blank).await.0, StatusCode::BAD_REQUEST);
    let long = json!({"reason": "r".repeat(501), "code": "000000"});
    assert_eq!(harness.post(&uri, long).await.0, StatusCode::BAD_REQUEST);

    let attempt = json!({"reason": "debugging", "code": "000000"});
    let (status, body) = harness.post(&uri, attempt.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("two factor"));

    let code = enable_two_factor(&harness).await;
    let (status, _) = harness
        .post(&uri, json!({"reason": "debugging", "code": "abcdef"}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, revealed) = harness
        .post(
            &uri,
            json!({"reason": "debugging", "code": code, "version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{revealed}");
    assert_eq!(revealed["value"], "s3cret");
    let (status, _) = harness
        .post(
            &uri,
            json!({"reason": "debugging", "code": code, "version": 9}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let results = entity::VaultAudit::find()
        .filter(entity::vault_audit::Column::Result.starts_with("denied"))
        .count(&harness.db)
        .await
        .unwrap();
    assert!(results >= 2);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn reveal_requiring_approval_needs_a_second_user() {
    let harness = full_harness().await;
    let (_, _, vault) = new_vault(&harness).await;
    put_secret(&harness, &vault, "TOKEN", "s3cret").await;
    let grant = json!({
        "principal": harness.admin_name,
        "capability": "reveal_secrets",
        "requires_approval": true,
    });
    assert_eq!(
        harness.post(&format!("{vault}/grants"), grant).await.0,
        StatusCode::CREATED
    );
    let code = enable_two_factor(&harness).await;
    let uri = format!("{vault}/secrets/TOKEN/reveal");
    let reveal = json!({"reason": "audit", "code": code});

    let (status, body) = harness.post(&uri, reveal.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "approval required");

    let approvals = format!("{vault}/approvals");
    let own = json!({"user_id": harness.admin_id, "secret_name": "TOKEN"});
    assert_eq!(
        harness.post(&approvals, own.clone()).await.0,
        StatusCode::FORBIDDEN
    );

    let (_, _, approver) = harness.user_with_token().await;
    let invalid = json!({"user_id": harness.admin_id, "secret_name": "bad name"});
    let denied = harness
        .send(Method::POST, &approvals, Some(&approver), Some(invalid))
        .await;
    assert_eq!(denied.0, StatusCode::BAD_REQUEST);
    let (status, approved) = harness
        .send(Method::POST, &approvals, Some(&approver), Some(own))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(approved["expires_at"].is_string());

    let (status, revealed) = harness.post(&uri, reveal).await;
    assert_eq!(status, StatusCode::OK, "{revealed}");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vault_routes_require_permissions() {
    let harness = full_harness().await;
    let (group, _, vault) = new_vault(&harness).await;
    let limited = harness.limited_token().await;
    let cases = [
        (Method::GET, base(group)),
        (Method::POST, base(group)),
        (Method::GET, vault.clone()),
        (Method::DELETE, vault.clone()),
        (Method::POST, format!("{vault}/seal")),
        (Method::GET, format!("{vault}/secrets")),
        (Method::POST, format!("{vault}/secrets")),
        (Method::GET, format!("{vault}/grants")),
        (Method::POST, format!("{vault}/approvals")),
        (Method::GET, format!("{vault}/audit")),
        (Method::POST, format!("{vault}/secrets/X/reveal")),
    ];
    for (method, uri) in cases {
        let body = Some(json!({"name": "x"}));
        let anonymous = harness.send(method.clone(), &uri, None, body.clone()).await;
        assert_eq!(anonymous.0, StatusCode::UNAUTHORIZED, "{uri}");
        let forbidden = harness.send(method, &uri, Some(&limited), body).await;
        assert_eq!(forbidden.0, StatusCode::FORBIDDEN, "{uri}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vault_requires_configured_secret_store() {
    let harness = crate::common::harness().await;
    let group = resource_group(&harness).await;
    let (status, _) = harness.post(&base(group), json!({"name": "nostore"})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
