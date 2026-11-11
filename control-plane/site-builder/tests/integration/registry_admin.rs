use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::common::{call, harness, insert_site, mock_registry, SECRET};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registry_endpoints_need_a_secret_and_a_registry() {
    let harness = harness(None, None).await;
    let uri = "/internal/registries/images";
    let body = Some(json!({ "prefix": "rg-1/" }));
    let (status, _) = call(&harness.app, Method::POST, uri, None, body.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, answer) = call(&harness.app, Method::POST, uri, Some(SECRET), body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(answer["error"]
        .as_str()
        .unwrap()
        .contains("no site registry"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn images_tags_and_prefixes_are_managed_through_the_registry() {
    let mock = mock_registry(&[("rg-9/web/app", &["v1", "v2"]), ("rg-9/web/db", &["v1"])]).await;
    let harness = harness(Some(&mock), None).await;
    let (status, body) = call(
        &harness.app,
        Method::POST,
        "/internal/registries/images",
        Some(SECRET),
        Some(json!({ "prefix": "rg-9/" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["repositories"].as_array().unwrap().len(), 2);
    let delete = json!({ "prefix": "rg-9/", "repository": "web/app", "tag": "v1" });
    let uri = "/internal/registries/delete-tag";
    let (status, _) = call(&harness.app, Method::POST, uri, Some(SECRET), Some(delete)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        mock.state.repositories.lock().unwrap()["rg-9/web/app"],
        vec!["v2"]
    );
    let invalid = json!({ "prefix": "rg-9/", "repository": "../escape", "tag": "v1" });
    let (status, _) = call(&harness.app, Method::POST, uri, Some(SECRET), Some(invalid)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let purge = Some(json!({ "prefix": "rg-9/" }));
    let (status, _) = call(
        &harness.app,
        Method::POST,
        "/internal/registries/purge",
        Some(SECRET),
        purge,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(mock.state.repositories.lock().unwrap()["rg-9/web/db"].is_empty());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn purging_a_site_removes_its_repository_tags() {
    let site_without_registry = harness(None, None).await;
    let site = insert_site(&site_without_registry.db, "container", Some(3000)).await;
    let uri = format!("/internal/sites/{}/purge", site.id);
    let (status, _) = call(
        &site_without_registry.app,
        Method::POST,
        &uri,
        Some(SECRET),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let name = site.id.to_string();
    let mock = mock_registry(&[(name.as_str(), &["a", "b"])]).await;
    let harness = harness(Some(&mock), None).await;
    let (status, _) = call(&harness.app, Method::POST, &uri, Some(SECRET), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(mock.state.repositories.lock().unwrap()[&name].is_empty());
}
