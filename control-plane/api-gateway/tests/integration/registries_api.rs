use axum::http::{Method, StatusCode};
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::Harness;
use crate::support_b::{builder_calls, full_harness, resource_group};

fn base(group: Uuid) -> String {
    format!("/api/resource-groups/{group}/registries")
}

fn registry_name() -> String {
    format!("reg-{}", &Uuid::new_v4().simple().to_string()[..12])
}

async fn create_registry(harness: &Harness, group: Uuid) -> (Value, String) {
    let (status, registry) = harness
        .post(&base(group), json!({"name": registry_name()}))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{registry}");
    let uri = format!("{}/{}", base(group), registry["id"].as_str().unwrap());
    (registry, uri)
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registries_are_created_listed_and_limited() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let (registry, uri) = create_registry(&harness, group).await;
    assert!(registry["prefix"]
        .as_str()
        .unwrap()
        .contains(registry["name"].as_str().unwrap()));

    let duplicate = json!({"name": registry["name"]});
    assert_eq!(
        harness.post(&base(group), duplicate).await.0,
        StatusCode::CONFLICT
    );
    for name in ["Bad Name", ""] {
        assert_eq!(
            harness.post(&base(group), json!({"name": name})).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let missing = base(Uuid::new_v4());
    assert_eq!(
        harness.post(&missing, json!({"name": "ok"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(harness.get(&missing).await.0, StatusCode::NOT_FOUND);

    let (status, detail) = harness.get(&uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["tokens"], json!([]));
    let unknown = format!("{}/{}", base(group), Uuid::new_v4());
    assert_eq!(harness.get(&unknown).await.0, StatusCode::NOT_FOUND);

    for _ in 0..9 {
        create_registry(&harness, group).await;
    }
    let (status, listed) = harness.get(&base(group)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 10);
    let extra = harness
        .post(&base(group), json!({"name": registry_name()}))
        .await;
    assert_eq!(extra.0, StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registry_tokens_are_issued_once_and_revocable() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let (_, uri) = create_registry(&harness, group).await;
    let tokens = format!("{uri}/tokens");

    let (status, token) = harness
        .post(&tokens, json!({"name": " ci ", "scope": "push"}))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(token["name"], "ci");
    assert!(token["secret"].as_str().unwrap().starts_with("csfxrt_"));
    assert!(token["username"].as_str().unwrap().starts_with("rt-"));

    for body in [
        json!({"name": "", "scope": "push"}),
        json!({"name": "x", "scope": "admin"}),
        json!({"name": "x".repeat(65), "scope": "pull"}),
    ] {
        assert_eq!(harness.post(&tokens, body).await.0, StatusCode::BAD_REQUEST);
    }

    let (_, detail) = harness.get(&uri).await;
    assert_eq!(detail["tokens"].as_array().unwrap().len(), 1);
    assert!(detail["tokens"][0].get("secret").is_none());

    let target = format!("{tokens}/{}", token["id"].as_str().unwrap());
    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn images_are_listed_relative_and_deleted_through_the_builder() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let (registry, uri) = create_registry(&harness, group).await;

    let (status, images) = harness.get(&format!("{uri}/images")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(images["repositories"][0]["repository"], "app/web");

    let target = format!("{uri}/images?repository=app/web&tag=v1");
    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NO_CONTENT
    );
    let deleted = builder_calls().into_iter().any(|(path, body)| {
        path == "/internal/registries/delete-tag"
            && body["tag"] == "v1"
            && body["prefix"] == registry["prefix"]
    });
    assert!(deleted);
    let incomplete = format!("{uri}/images?repository=app/web");
    assert_eq!(
        harness.admin(Method::DELETE, &incomplete, None).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn deleting_a_registry_purges_images() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let (registry, uri) = create_registry(&harness, group).await;

    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    let purged = builder_calls().into_iter().any(|(path, body)| {
        path == "/internal/registries/purge" && body["prefix"] == registry["prefix"]
    });
    assert!(purged);
    assert_eq!(harness.get(&uri).await.0, StatusCode::NOT_FOUND);
    let id = registry["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    assert!(entity::ImageRegistries::find_by_id(id)
        .one(&harness.db)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registry_routes_require_permissions_and_a_builder() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let (_, uri) = create_registry(&harness, group).await;
    let limited = harness.limited_token().await;
    let cases = [
        (Method::GET, base(group)),
        (Method::POST, base(group)),
        (Method::GET, uri.clone()),
        (Method::DELETE, uri.clone()),
        (Method::GET, format!("{uri}/images")),
        (Method::DELETE, format!("{uri}/images")),
        (Method::POST, format!("{uri}/tokens")),
    ];
    for (method, target) in cases {
        let body = Some(json!({}));
        let anonymous = harness
            .send(method.clone(), &target, None, body.clone())
            .await;
        assert_eq!(anonymous.0, StatusCode::UNAUTHORIZED, "{target}");
        let denied = harness.send(method, &target, Some(&limited), body).await;
        assert_eq!(denied.0, StatusCode::FORBIDDEN, "{target}");
    }

    let bare = crate::common::harness().await;
    let (status, _) = bare.get(&format!("{uri}/images")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
