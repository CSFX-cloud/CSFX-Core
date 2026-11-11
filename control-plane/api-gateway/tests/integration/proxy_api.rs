use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::harness;

struct Case {
    method: Method,
    uri: String,
    backend_path: String,
    body: Option<Value>,
}

fn case(method: Method, uri: &str, backend_path: &str, body: Option<Value>) -> Case {
    Case {
        method,
        uri: format!("/api{}", uri),
        backend_path: backend_path.to_string(),
        body,
    }
}

fn forwarded_cases() -> Vec<Case> {
    let id = Uuid::new_v4();
    let member = Uuid::new_v4();
    let payload = Some(json!({"name": "demo", "image": "nginx:1"}));
    let at = |template: &str| template.replace("{id}", &id.to_string());
    let mut cases = Vec::new();
    for (method, suffix, body) in [
        (Method::DELETE, "", None),
        (Method::PATCH, "", payload.clone()),
        (Method::POST, "/stop", None),
        (Method::POST, "/restart", None),
    ] {
        let uri = at(&format!("/workloads/{{id}}{}", suffix));
        cases.push(case(method, &uri, &uri, body));
    }
    cases.push(case(
        Method::POST,
        "/workloads",
        "/workloads",
        payload.clone(),
    ));
    cases.push(case(Method::GET, "/workloads", "/workloads", None));
    cases.push(case(
        Method::POST,
        "/workload-stacks",
        "/workload-stacks",
        payload.clone(),
    ));
    for (method, suffix, body) in [
        (Method::GET, "", None),
        (Method::DELETE, "", None),
        (
            Method::PATCH,
            "",
            Some(json!({"compose_yaml": "services: {}"})),
        ),
        (Method::POST, "/stop", None),
        (Method::POST, "/restart", None),
    ] {
        let uri = at(&format!("/workload-stacks/{{id}}{}", suffix));
        cases.push(case(method, &uri, &uri, body));
    }
    cases.push(case(Method::POST, "/volumes", "/volumes", payload.clone()));
    cases.push(case(Method::GET, "/volumes", "/volumes", None));
    for (method, suffix, body) in [
        (Method::GET, "", None),
        (Method::DELETE, "", None),
        (
            Method::POST,
            "/attach",
            Some(json!({"workload_id": member})),
        ),
        (Method::POST, "/detach", None),
        (Method::POST, "/snapshots", Some(json!({"name": "snap"}))),
        (Method::GET, "/snapshots", None),
    ] {
        let uri = at(&format!("/volumes/{{id}}{}", suffix));
        cases.push(case(method, &uri, &uri, body));
    }
    cases.push(case(Method::POST, "/networks", "/networks", payload));
    cases.push(case(Method::GET, "/networks", "/networks", None));
    for (method, suffix, body) in [
        (Method::GET, "", None),
        (Method::DELETE, "", None),
        (Method::GET, "/policies", None),
        (Method::POST, "/policies", Some(json!({"action": "allow"}))),
        (Method::GET, "/members", None),
        (
            Method::POST,
            "/members",
            Some(json!({"workload_id": member})),
        ),
        (Method::DELETE, &format!("/members/{}", member), None),
    ] {
        let uri = at(&format!("/networks/{{id}}{}", suffix));
        cases.push(case(method, &uri, &uri, body));
    }
    cases.push(case(Method::GET, "/events", "/events", None));
    cases.push(case(Method::GET, "/events?limit=5", "/events", None));
    let resolve = at("/events/{id}/resolve");
    cases.push(case(Method::POST, &resolve, &resolve, None));
    let maintenance = at("/agents/{id}/maintenance");
    cases.push(case(
        Method::POST,
        &maintenance,
        &maintenance,
        Some(json!({"until": null})),
    ));
    cases.push(case(Method::DELETE, &maintenance, &maintenance, None));
    cases
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn requests_are_forwarded_to_the_owning_service() {
    let harness = harness().await;

    for case in forwarded_cases() {
        let (status, body) = harness
            .admin(case.method.clone(), &case.uri, case.body.clone())
            .await;
        assert_eq!(status, StatusCode::OK, "{} {}", case.method, case.uri);
        assert_eq!(body["method"], case.method.as_str(), "{}", case.uri);
        assert_eq!(body["path"], case.backend_path, "{}", case.uri);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn forwarded_routes_enforce_authentication_and_permissions() {
    let harness = harness().await;
    let limited = harness.limited_token().await;

    for case in forwarded_cases() {
        let (status, _) = harness
            .send(case.method.clone(), &case.uri, None, case.body.clone())
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{}", case.uri);
        let (status, _) = harness
            .send(
                case.method.clone(),
                &case.uri,
                Some(&limited),
                case.body.clone(),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{} {}",
            case.method,
            case.uri
        );
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn workload_creation_rejects_reserved_and_foreign_images() {
    let harness = harness().await;
    let own = Uuid::new_v4().simple();
    let other = Uuid::new_v4().simple();
    let bodies = [
        json!({"image": "sites.csfx.invalid/app:1"}),
        json!({"compose_yaml": "services:\n  a:\n    image: sites.csfx.invalid/app:1\n"}),
        json!({"image": format!("10.0.0.5:5060/rg-{}/main/app:1", other), "resource_group_id": own.to_string()}),
    ];

    for body in bodies {
        for uri in ["/api/workloads", "/api/workload-stacks"] {
            let (status, error) = harness.post(uri, body.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{} {}", uri, body);
            assert!(error["error"].is_string());
        }
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn workload_creation_accepts_the_groups_own_registry() {
    let harness = harness().await;
    let own = Uuid::new_v4();

    let (status, _) = harness
        .post(
            "/api/workloads",
            json!({
                "image": format!("10.0.0.5:5060/rg-{}/main/app:1", own.simple()),
                "resource_group_id": own.to_string()
            }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn updates_check_the_stored_group_of_the_target() {
    let harness = harness().await;
    let foreign = Uuid::new_v4().simple();
    let unknown = Uuid::new_v4();

    let (status, _) = harness
        .admin(
            Method::PATCH,
            &format!("/api/workloads/{}", unknown),
            Some(json!({"image": format!("host/rg-{}/main/app:1", foreign)})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = harness
        .admin(
            Method::PATCH,
            &format!("/api/workload-stacks/{}", unknown),
            Some(json!({"compose_yaml": format!("services:\n  a:\n    image: host/rg-{}/x/app\n", foreign)})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
