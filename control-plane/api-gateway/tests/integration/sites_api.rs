use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use crate::common::Harness;
use crate::support_b::{builder_calls, full_harness, resource_group};

const STATIC_REPO: &str = "https://git.test/org/static-site";
const CONTAINER_REPO: &str = "https://git.test/org/container-app";

fn sites_uri(group: Uuid) -> String {
    format!("/api/resource-groups/{group}/sites")
}

fn site_name() -> String {
    format!("site-{}", &Uuid::new_v4().simple().to_string()[..12])
}

async fn create_site(harness: &Harness, group: Uuid, repo: &str) -> Value {
    let body = json!({"name": site_name(), "repo_url": repo});
    let (status, site) = harness.post(&sites_uri(group), body).await;
    assert_eq!(status, StatusCode::CREATED, "{site}");
    site
}

fn site_uri(group: Uuid, site: &Value) -> String {
    format!("{}/{}", sites_uri(group), site["id"].as_str().unwrap())
}

async fn call_hook(
    harness: &Harness,
    uri: &str,
    event: &str,
    payload: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-github-event", event)
        .body(Body::from(payload.to_string()))
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:5000".parse::<SocketAddr>().unwrap()));
    let response = harness.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn webhook_uri(harness: &Harness, site: &Value) -> String {
    let model = entity::Sites::find_by_id(site["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap();
    format!("/api/hooks/sites/{}/{}", model.id, model.webhook_secret)
}

fn push(branch: &str, sha: &str) -> Value {
    json!({
        "ref": format!("refs/heads/{branch}"),
        "after": sha,
        "head_commit": {"message": "ship it\nbody"},
    })
}

fn pull_request(action: &str, same_repo: bool) -> Value {
    let head = if same_repo { "org/repo" } else { "fork/repo" };
    json!({
        "action": action,
        "pull_request": {
            "number": 7,
            "title": "feature",
            "head": {"ref": "feature", "sha": "def456", "repo": {"full_name": head}},
            "base": {"repo": {"full_name": "org/repo"}},
        },
    })
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn detect_reports_framework_and_validates_input() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let uri = format!("{}/detect", sites_uri(group));

    let (status, body) = harness.post(&uri, json!({"repo_url": STATIC_REPO})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detection"]["framework"], "vite");
    assert_eq!(body["detection"]["mode"], "static");

    let none = json!({"repo_url": "https://git.test/undetectable"});
    assert_eq!(harness.post(&uri, none).await.1["detection"], Value::Null);

    for body in [
        json!({"repo_url": "git@host:a/b"}),
        json!({"repo_url": STATIC_REPO, "branch": "--evil"}),
        json!({"repo_url": STATIC_REPO, "root_dir": "../x"}),
    ] {
        assert_eq!(harness.post(&uri, body).await.0, StatusCode::BAD_REQUEST);
    }
    let rejected = json!({"repo_url": "https://git.test/builder-reject"});
    assert_eq!(
        harness.post(&uri, rejected).await.0,
        StatusCode::BAD_GATEWAY
    );
    let missing = format!("{}/detect", sites_uri(Uuid::new_v4()));
    let ok = json!({"repo_url": STATIC_REPO});
    assert_eq!(harness.post(&missing, ok).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn create_site_persists_domains_and_queues_a_build() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    assert_eq!(site["mode"], "static");
    assert_eq!(site["output_dir"], "dist");
    assert!(site.get("webhook_secret").is_none());

    let (status, detail) = harness.get(&site_uri(group, &site)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["deployments"].as_array().unwrap().len(), 1);
    assert_eq!(detail["deployments"][0]["status"], "queued");
    let hostname = format!("{}.svc.{group}.internal", site["name"].as_str().unwrap());
    assert_eq!(detail["domains"][0]["hostname"], hostname);

    let queued = builder_calls().into_iter().any(|(path, body)| {
        path == "/internal/builds" && body["site_id"] == site["id"] && body["env"] == json!({})
    });
    assert!(queued);

    let (status, listed) = harness.get(&sites_uri(group)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["id"], site["id"]);
    assert_eq!(listed[0]["domains"], json!([]));
    assert_eq!(listed[0]["latest"]["environment"], "production");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn create_site_validates_and_resolves_modes() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let uri = sites_uri(group);
    let repo = json!(STATIC_REPO);

    for body in [
        json!({"name": "Bad_Name", "repo_url": repo}),
        json!({"name": site_name(), "repo_url": repo, "build_command": "a\0b"}),
        json!({"name": site_name(), "repo_url": repo, "gateway_id": Uuid::new_v4()}),
        json!({"name": site_name(), "repo_url": "https://git.test/undetectable"}),
        json!({"name": site_name(), "repo_url": "https://git.test/undetectable", "mode": "container"}),
    ] {
        assert_eq!(harness.post(&uri, body).await.0, StatusCode::BAD_REQUEST);
    }
    let manual = json!({
        "name": site_name(), "repo_url": "https://git.test/undetectable",
        "mode": "static", "output_dir": "public",
    });
    assert_eq!(harness.post(&uri, manual).await.0, StatusCode::CREATED);

    let container = create_site(&harness, group, CONTAINER_REPO).await;
    assert_eq!(container["mode"], "container");
    assert_eq!(container["container_port"], 3000);
    let (_, detail) = harness.get(&site_uri(group, &container)).await;
    assert_eq!(detail["domains"], json!([]));

    let duplicate = json!({"name": container["name"], "repo_url": repo});
    assert_eq!(harness.post(&uri, duplicate).await.0, StatusCode::CONFLICT);
    let missing = sites_uri(Uuid::new_v4());
    let named = json!({"name": site_name(), "repo_url": repo});
    assert_eq!(harness.post(&missing, named).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn update_site_applies_and_clears_fields() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let uri = site_uri(group, &site);

    let patch = json!({
        "production_branch": "release", "root_dir": "web", "build_command": "make",
        "install_command": "  ", "container_port": 8080, "preview_enabled": false,
        "framework": "custom", "output_dir": "out", "start_command": "run",
        "git_token_secret": "TOKEN",
    });
    let (status, updated) = harness.admin(Method::PUT, &uri, Some(patch)).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["production_branch"], "release");
    assert_eq!(updated["build_command"], "make");
    assert_eq!(updated["install_command"], Value::Null);
    assert_eq!(updated["container_port"], 8080);
    assert_eq!(updated["preview_enabled"], false);

    for bad in [
        json!({"production_branch": "--x"}),
        json!({"root_dir": "/abs"}),
        json!({"start_command": "a\0b"}),
        json!({"gateway_id": Uuid::new_v4()}),
    ] {
        let (status, _) = harness.admin(Method::PUT, &uri, Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let missing = site_uri(group, &json!({"id": Uuid::new_v4()}));
    let (status, _) = harness.admin(Method::PUT, &missing, Some(json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn environment_variables_validate_and_resolve_into_builds() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let env = format!("{}/env", site_uri(group, &site));

    let (status, created) = harness
        .post(
            &env,
            json!({"name": "API_URL", "value": "https://api.test", "scope": "production"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let same = json!({"name": "API_URL", "value": "x", "scope": "production"});
    assert_eq!(harness.post(&env, same).await.0, StatusCode::CONFLICT);

    for body in [
        json!({"name": "1bad", "value": "x"}),
        json!({"name": "OK_NAME", "value": "x", "scope": "staging"}),
        json!({"name": "OK_NAME"}),
        json!({"name": "OK_NAME", "value": "x", "vault_secret": "S"}),
        json!({"name": "OK_NAME", "vault_secret": "S"}),
    ] {
        assert_eq!(harness.post(&env, body).await.0, StatusCode::BAD_REQUEST);
    }

    let (_, detail) = harness.get(&site_uri(group, &site)).await;
    assert_eq!(detail["env"][0]["name"], "API_URL");

    let (status, _) = harness
        .post(
            &format!("{}/deployments", site_uri(group, &site)),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let carried = builder_calls().into_iter().any(|(path, body)| {
        path == "/internal/builds"
            && body["site_id"] == site["id"]
            && body["env"]["API_URL"] == "https://api.test"
    });
    assert!(carried);

    let target = format!("{env}/{}", created["id"].as_str().unwrap());
    let removed = harness.admin(Method::DELETE, &target, None).await;
    assert_eq!(removed.0, StatusCode::NO_CONTENT);
    let again = harness.admin(Method::DELETE, &target, None).await;
    assert_eq!(again.0, StatusCode::NOT_FOUND);
}

async fn vault_with_secret(harness: &Harness, group: Uuid, secret: &str) -> Uuid {
    let (_, vault) = harness
        .post(
            &format!("/api/resource-groups/{group}/vaults"),
            json!({"name": format!("v-{}", Uuid::new_v4().simple())}),
        )
        .await;
    let id = vault["id"].as_str().unwrap().parse().unwrap();
    let uri = format!("/api/resource-groups/{group}/vaults/{id}/secrets");
    let body = json!({"name": secret, "value": "vault-value"});
    assert_eq!(harness.post(&uri, body).await.0, StatusCode::CREATED);
    id
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vault_backed_variables_and_git_tokens_are_fetched() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let vault_id = vault_with_secret(&harness, group, "GIT_TOKEN").await;
    let body = json!({
        "name": site_name(), "repo_url": STATIC_REPO,
        "vault_id": vault_id, "git_token_secret": "GIT_TOKEN",
    });
    let (status, site) = harness.post(&sites_uri(group), body).await;
    assert_eq!(status, StatusCode::CREATED, "{site}");
    let token = builder_calls().into_iter().any(|(path, body)| {
        path == "/internal/builds"
            && body["site_id"] == site["id"]
            && body["git_token"] == "vault-value"
    });
    assert!(token);

    let env = format!("{}/env", site_uri(group, &site));
    let secret = json!({"name": "DB_PASS", "vault_secret": "GIT_TOKEN"});
    assert_eq!(harness.post(&env, secret).await.0, StatusCode::CREATED);
    let redeploy = format!("{}/deployments", site_uri(group, &site));
    assert_eq!(
        harness.post(&redeploy, json!({})).await.0,
        StatusCode::ACCEPTED
    );
    let resolved = builder_calls()
        .into_iter()
        .any(|(path, body)| path == "/internal/builds" && body["env"]["DB_PASS"] == "vault-value");
    assert!(resolved);

    let missing = json!({"name": "MISSING", "vault_secret": "NOPE"});
    assert_eq!(harness.post(&env, missing).await.0, StatusCode::CREATED);
    let (status, _) = harness.post(&redeploy, json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let failed = entity::SiteDeployments::find()
        .filter(entity::site_deployments::Column::Status.eq("failed"))
        .count(&harness.db)
        .await
        .unwrap();
    assert!(failed >= 1);

    let unknown = json!({"name": site_name(), "repo_url": STATIC_REPO, "git_token_secret": "X"});
    assert_eq!(
        harness.post(&sites_uri(group), unknown).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn git_token_secrets_require_the_reveal_permission() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let vault_id = vault_with_secret(&harness, group, "GIT_TOKEN").await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let (_, manager) = harness
        .token_with_permissions(&["resource_groups.manage"])
        .await;
    let token = json!({"vault_id": vault_id, "git_token_secret": "GIT_TOKEN"});
    let detect = format!("{}/detect", sites_uri(group));
    let create = json!({"name": site_name(), "repo_url": STATIC_REPO,
        "vault_id": vault_id, "git_token_secret": "GIT_TOKEN"});
    let detect_body = json!({"repo_url": STATIC_REPO,
        "vault_id": vault_id, "git_token_secret": "GIT_TOKEN"});
    let cases = [
        (Method::POST, detect, detect_body),
        (Method::POST, sites_uri(group), create),
        (Method::PUT, site_uri(group, &site), token),
    ];
    for (method, target, body) in cases {
        let (status, _) = harness
            .send(method, &target, Some(&manager), Some(body))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{target}");
    }
    let clear = json!({"git_token_secret": ""});
    let (status, _) = harness
        .send(
            Method::PUT,
            &site_uri(group, &site),
            Some(&manager),
            Some(clear),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn the_webhook_path_is_only_shown_to_managers() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let target = site_uri(group, &site);
    let (_, viewer) = harness
        .token_with_permissions(&["resource_groups.view"])
        .await;
    let (_, manager) = harness
        .token_with_permissions(&["resource_groups.view", "resource_groups.manage"])
        .await;

    let (status, body) = harness
        .send(Method::GET, &target, Some(&viewer), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("webhook_path").is_none());
    assert!(!body.to_string().contains("/hooks/sites/"));

    let (status, body) = harness
        .send(Method::GET, &target, Some(&manager), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["webhook_path"]
        .as_str()
        .unwrap()
        .starts_with("/hooks/sites/"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn domains_are_validated_and_unique() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, CONTAINER_REPO).await;
    let domains = format!("{}/domains", site_uri(group, &site));
    let hostname = format!("{}.example.test", Uuid::new_v4().simple());

    let (status, domain) = harness.post(&domains, json!({"hostname": hostname})).await;
    assert_eq!(status, StatusCode::CREATED, "{domain}");
    assert_eq!(
        harness
            .post(&domains, json!({"hostname": hostname}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        harness
            .post(&domains, json!({"hostname": "not a host"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let reconciled = builder_calls().into_iter().any(|(path, _)| {
        path == format!("/internal/sites/{}/reconcile", site["id"].as_str().unwrap())
    });
    assert!(reconciled);

    let target = format!("{domains}/{}", domain["id"].as_str().unwrap());
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
async fn internal_domains_cannot_be_deleted() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let (_, detail) = harness.get(&site_uri(group, &site)).await;
    let internal = detail["domains"][0]["id"].as_str().unwrap();
    let target = format!("{}/domains/{internal}", site_uri(group, &site));
    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn redeploy_and_promote_deployments() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let deployments = format!("{}/deployments", site_uri(group, &site));

    let (status, preview) = harness
        .post(&deployments, json!({"branch": "feature/x"}))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(preview["environment"], "preview");
    assert_eq!(
        harness
            .post(&deployments, json!({"branch": "--bad"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    let one = format!("{deployments}/{}", preview["id"].as_str().unwrap());
    let (status, fetched) = harness.get(&one).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["branch"], "feature/x");
    let unknown = format!("{deployments}/{}", Uuid::new_v4());
    assert_eq!(harness.get(&unknown).await.0, StatusCode::NOT_FOUND);

    assert_eq!(
        harness.post(&format!("{one}/promote"), json!({})).await.0,
        StatusCode::OK
    );
    let promoted = builder_calls().into_iter().any(|(path, _)| {
        path == format!(
            "/internal/deployments/{}/promote",
            preview["id"].as_str().unwrap()
        )
    });
    assert!(promoted);
    let missing = format!("{unknown}/promote");
    assert_eq!(
        harness.post(&missing, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn container_sites_only_deploy_production_and_promote_with_env() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, CONTAINER_REPO).await;
    let deployments = format!("{}/deployments", site_uri(group, &site));

    let other = harness.post(&deployments, json!({"branch": "dev"})).await;
    assert_eq!(other.0, StatusCode::BAD_REQUEST);
    let (status, deployment) = harness.post(&deployments, json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let promote = format!(
        "{deployments}/{}/promote",
        deployment["id"].as_str().unwrap()
    );
    assert_eq!(harness.post(&promote, json!({})).await.0, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn delete_site_removes_rows_and_purges_images() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let uri = site_uri(group, &site);

    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(harness.get(&uri).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::NOT_FOUND
    );
    let purged = builder_calls()
        .into_iter()
        .any(|(path, _)| path == format!("/internal/sites/{}/purge", site["id"].as_str().unwrap()));
    assert!(purged);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn webhooks_queue_matching_events_only() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let uri = webhook_uri(&harness, &site).await;

    let (status, body) = call_hook(&harness, &uri, "push", push("main", "abc123")).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(body["deployment_id"].is_string());
    let (status, body) =
        call_hook(&harness, &uri, "pull_request", pull_request("opened", true)).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");

    let ignored = [
        ("push", push("other", "abc123")),
        ("push", push("main", &"0".repeat(40))),
        ("pull_request", pull_request("closed", true)),
        ("pull_request", pull_request("opened", false)),
        ("ping", json!({})),
    ];
    for (event, payload) in ignored {
        let (status, body) = call_hook(&harness, &uri, event, payload).await;
        assert_eq!(status, StatusCode::OK, "{event}");
        assert_eq!(body["ignored"], true);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn webhooks_reject_unknown_sites_and_wrong_secrets() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let valid = webhook_uri(&harness, &site).await;
    let wrong = format!("{}wrong", valid);
    let unknown = format!("/api/hooks/sites/{}/secret", Uuid::new_v4());

    for uri in [wrong, unknown] {
        let (status, _) = call_hook(&harness, &uri, "push", push("main", "abc")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn site_routes_require_permissions() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let site = create_site(&harness, group, STATIC_REPO).await;
    let limited = harness.limited_token().await;
    let uri = site_uri(group, &site);
    let cases = [
        (Method::GET, sites_uri(group)),
        (Method::POST, sites_uri(group)),
        (Method::POST, format!("{}/detect", sites_uri(group))),
        (Method::GET, uri.clone()),
        (Method::PUT, uri.clone()),
        (Method::DELETE, uri.clone()),
        (Method::POST, format!("{uri}/deployments")),
        (Method::POST, format!("{uri}/env")),
        (Method::POST, format!("{uri}/domains")),
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
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn sites_report_unavailable_without_a_builder() {
    let harness = crate::common::harness().await;
    let group = resource_group(&harness).await;
    let body = json!({"name": site_name(), "repo_url": STATIC_REPO});
    let (status, _) = harness.post(&sites_uri(group), body).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
