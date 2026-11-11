use axum::http::{Method, StatusCode};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use site_builder::token::deployment_token;
use uuid::Uuid;

use crate::common::{
    call, deployment_of, harness, insert_deployment, insert_domain, insert_site, job_body,
    mock_registry, request, routes_for, site_of, tar_gz, Harness, SECRET,
};

fn callback(id: Uuid) -> String {
    deployment_token(SECRET, id)
}

async fn put_artifact(harness: &Harness, id: Uuid, body: Vec<u8>) -> StatusCode {
    let token = format!("Bearer {}", callback(id));
    let uri = format!("/callbacks/{id}/artifact");
    let headers = [("authorization", token.as_str())];
    request(&harness.app, Method::PUT, &uri, &headers, body)
        .await
        .0
}

async fn complete(harness: &Harness, id: Uuid, body: serde_json::Value) -> StatusCode {
    let uri = format!("/callbacks/{id}/complete");
    call(
        &harness.app,
        Method::POST,
        &uri,
        Some(&callback(id)),
        Some(body),
    )
    .await
    .0
}

async fn start(harness: &Harness, body: serde_json::Value) -> StatusCode {
    call(
        &harness.app,
        Method::POST,
        "/internal/builds",
        Some(SECRET),
        Some(body),
    )
    .await
    .0
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn starting_a_build_moves_the_deployment_to_building_once() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let deployment = insert_deployment(&harness.db, site.id, "queued", "production", "main").await;
    let body = job_body(&site, &deployment);
    assert_eq!(start(&harness, body.clone()).await, StatusCode::ACCEPTED);
    let stored = deployment_of(&harness.db, deployment.id).await;
    assert_eq!(stored.status, "building");
    assert!(stored.workload_id.is_some());
    let created = harness.scheduler.created.lock().unwrap().clone();
    assert_eq!(created[0]["env_vars"]["CSFX_MODE"], "static");
    assert_eq!(start(&harness, body).await, StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn invalid_unknown_and_unauthorized_builds_are_refused() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let deployment = insert_deployment(&harness.db, site.id, "queued", "production", "main").await;
    let mut invalid = job_body(&site, &deployment);
    invalid["branch"] = "bad branch".into();
    assert_eq!(start(&harness, invalid).await, StatusCode::BAD_REQUEST);
    let mut unknown = job_body(&site, &deployment);
    unknown["deployment_id"] = Uuid::new_v4().to_string().into();
    assert_eq!(start(&harness, unknown).await, StatusCode::NOT_FOUND);
    let body = job_body(&site, &deployment);
    let (status, _) = call(
        &harness.app,
        Method::POST,
        "/internal/builds",
        None,
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn scheduler_rejections_fail_the_deployment() {
    let harness = harness(None, None).await;
    harness
        .scheduler
        .reject
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let site = insert_site(&harness.db, "static", None).await;
    let deployment = insert_deployment(&harness.db, site.id, "queued", "production", "main").await;
    assert_eq!(
        start(&harness, job_body(&site, &deployment)).await,
        StatusCode::BAD_GATEWAY
    );
    let stored = deployment_of(&harness.db, deployment.id).await;
    assert_eq!(stored.status, "failed");
    assert!(stored
        .build_log
        .contains("[ERROR] scheduler error: no capacity"));
    assert!(stored.finished_at.is_some());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn callbacks_check_tokens_and_deployment_state() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let deployment =
        insert_deployment(&harness.db, site.id, "building", "production", "main").await;
    let id = deployment.id;
    let uri = format!("/callbacks/{id}/log");
    let (status, _) = call(&harness.app, Method::POST, &uri, Some("00"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let token = format!("Bearer {}", callback(id));
    let headers = [("authorization", token.as_str())];
    request(
        &harness.app,
        Method::POST,
        &uri,
        &headers,
        b"first line\n".to_vec(),
    )
    .await;
    let (status, _) = request(
        &harness.app,
        Method::POST,
        &uri,
        &headers,
        b"second\n".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        deployment_of(&harness.db, id).await.build_log,
        "first line\nsecond\n"
    );
    assert_eq!(
        put_artifact(&harness, id, b"not a tarball".to_vec()).await,
        StatusCode::BAD_REQUEST
    );
    let queued = insert_deployment(&harness.db, site.id, "queued", "production", "main").await;
    let archive = tar_gz(&[("index.html", b"x")]);
    assert_eq!(
        put_artifact(&harness, queued.id, archive).await,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn a_finished_static_production_build_goes_live() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let deployment = insert_deployment(&harness.db, site.id, "queued", "production", "main").await;
    start(&harness, job_body(&site, &deployment)).await;
    let archive = tar_gz(&[("index.html", b"<h1>hi</h1>")]);
    assert_eq!(
        put_artifact(&harness, deployment.id, archive).await,
        StatusCode::NO_CONTENT
    );
    let report = serde_json::json!({ "success": true, "commit_sha": "feedface", "commit_message": "ship it" });
    assert_eq!(
        complete(&harness, deployment.id, report).await,
        StatusCode::NO_CONTENT
    );
    let stored = deployment_of(&harness.db, deployment.id).await;
    assert_eq!(
        (stored.status.as_str(), stored.commit_sha.as_str()),
        ("ready", "feedface")
    );
    assert_eq!(stored.artifact_key, Some(deployment.id.to_string()));
    assert_eq!(
        site_of(&harness.db, site.id).await.current_deployment_id,
        Some(deployment.id)
    );
    assert_eq!(harness.scheduler.deleted.lock().unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn failed_reports_and_missing_artifacts_fail_the_deployment() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let reported = insert_deployment(&harness.db, site.id, "building", "production", "main").await;
    let report = serde_json::json!({ "success": false, "error": "npm exploded" });
    assert_eq!(
        complete(&harness, reported.id, report).await,
        StatusCode::NO_CONTENT
    );
    let stored = deployment_of(&harness.db, reported.id).await;
    assert_eq!(stored.status, "failed");
    assert!(stored.build_log.contains("npm exploded"));
    let again = serde_json::json!({ "success": true });
    assert_eq!(
        complete(&harness, reported.id, again).await,
        StatusCode::CONFLICT
    );
    let empty = insert_deployment(&harness.db, site.id, "building", "production", "main").await;
    let success = serde_json::json!({ "success": true });
    assert_eq!(
        complete(&harness, empty.id, success).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(deployment_of(&harness.db, empty.id).await.status, "failed");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn preview_builds_register_a_branch_domain() {
    let harness = harness(None, Some("preview.test")).await;
    let site = insert_site(&harness.db, "static", None).await;
    let deployment =
        insert_deployment(&harness.db, site.id, "queued", "preview", "Feature/Login").await;
    start(&harness, job_body(&site, &deployment)).await;
    put_artifact(&harness, deployment.id, tar_gz(&[("index.html", b"x")])).await;
    let status = complete(
        &harness,
        deployment.id,
        serde_json::json!({ "success": true }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let hostname = format!("feature-login-{}.preview.test", site.name);
    let domain = entity::SiteDomains::find()
        .filter(entity::site_domains::Column::Hostname.eq(hostname))
        .one(&harness.db)
        .await
        .unwrap();
    assert_eq!(domain.unwrap().kind, "branch");
    assert_eq!(
        site_of(&harness.db, site.id).await.current_deployment_id,
        None
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn container_builds_publish_the_trusted_image_and_routes() {
    let mock = mock_registry(&[]).await;
    let harness = harness(Some(&mock), None).await;
    let site = insert_site(&harness.db, "container", Some(3000)).await;
    let hostname = insert_domain(&harness.db, site.id, "production", None).await;
    let deployment = insert_deployment(&harness.db, site.id, "queued", "production", "main").await;
    assert_eq!(
        start(&harness, job_body(&site, &deployment)).await,
        StatusCode::ACCEPTED
    );
    let report = serde_json::json!({ "success": true, "image_ref": "evil/other:1" });
    assert_eq!(
        complete(&harness, deployment.id, report).await,
        StatusCode::NO_CONTENT
    );
    let stored = deployment_of(&harness.db, deployment.id).await;
    let image = stored.image_ref.unwrap();
    assert!(image.starts_with(&format!("sites.csfx.invalid/{}:", site.id)));
    assert!(stored.workload_id.is_some());
    let routes = routes_for(&harness.db, &hostname).await;
    assert_eq!(routes.len(), 1);
    assert_eq!(Some(routes[0].workload_id), stored.workload_id);
    let created = harness.scheduler.created.lock().unwrap().clone();
    assert_eq!(created[1]["env_vars"]["API_URL"], "https://api.test");
    assert_eq!(created[1]["env_vars"]["PORT"], "3000");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn container_builds_without_a_report_or_job_state_fail() {
    let mock = mock_registry(&[]).await;
    let harness = harness(Some(&mock), None).await;
    let site = insert_site(&harness.db, "container", Some(3000)).await;
    let silent = insert_deployment(&harness.db, site.id, "building", "production", "main").await;
    let status = complete(&harness, silent.id, serde_json::json!({ "success": true })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let forgotten = insert_deployment(&harness.db, site.id, "building", "production", "main").await;
    let report = serde_json::json!({ "success": true, "image_ref": "x" });
    assert_eq!(
        complete(&harness, forgotten.id, report).await,
        StatusCode::BAD_REQUEST
    );
    let stored = deployment_of(&harness.db, forgotten.id).await;
    assert!(stored.build_log.contains("build job state was lost"));
}
