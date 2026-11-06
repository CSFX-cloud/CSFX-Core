use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::harness;

const ADMIN_CASES: [(&str, &str, &str); 12] = [
    ("POST", "/agents/pre-register", "/admin/agents/pre-register"),
    ("GET", "/agents/pending", "/admin/agents/pending"),
    ("POST", "/agents/pending/{id}", "/admin/agents/pending/{id}"),
    ("POST", "/bootstrap-tokens", "/admin/bootstrap-tokens"),
    ("GET", "/bootstrap-tokens", "/admin/bootstrap-tokens"),
    (
        "POST",
        "/bootstrap-tokens/{id}/revoke",
        "/admin/bootstrap-tokens/{id}/revoke",
    ),
    ("POST", "/tokens", "/admin/tokens"),
    ("GET", "/tokens", "/admin/tokens"),
    ("GET", "/agents", "/admin/agents"),
    ("GET", "/statistics", "/admin/statistics"),
    ("POST", "/agents/{id}/revoke", "/admin/agents/{id}/revoke"),
    (
        "GET",
        "/agents/{id}/endpoint",
        "/admin/agents/{id}/endpoint",
    ),
];

const AGENT_CASES: [(&str, &str, &str); 5] = [
    ("POST", "/agents/register", "/agents/register"),
    ("POST", "/agents/{id}/heartbeat", "/agents/{id}/heartbeat"),
    (
        "POST",
        "/agents/{id}/certificate",
        "/agents/{id}/certificate",
    ),
    (
        "POST",
        "/agents/{id}/rotate-certificate",
        "/agents/{id}/rotate-certificate",
    ),
    ("GET", "/pki/crl", "/pki/crl"),
];

fn resolve(template: &str, id: Uuid) -> String {
    template.replace("{id}", &id.to_string())
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn admin_routes_require_authentication_and_forward() {
    let harness = harness().await;
    let id = Uuid::new_v4();

    for (method, path, backend) in ADMIN_CASES {
        let method: Method = method.parse().unwrap();
        let uri = format!("/api/registry/admin{}", resolve(path, id));
        let body = Some(json!({"name": "node", "hostname": "node.test"}));

        let (status, _) = harness.send(method.clone(), &uri, None, body.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{}", uri);

        let (status, echoed) = harness.admin(method.clone(), &uri, body).await;
        assert_eq!(status, StatusCode::OK, "{}", uri);
        assert_eq!(echoed["path"], resolve(backend, id), "{}", uri);
        assert_eq!(echoed["method"], method.as_str(), "{}", uri);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_routes_are_forwarded_without_a_user_session() {
    let harness = harness().await;
    let id = Uuid::new_v4();

    for (method, path, backend) in AGENT_CASES {
        let method: Method = method.parse().unwrap();
        let uri = format!("/api/registry{}", resolve(path, id));

        let (status, echoed) = harness
            .send(method.clone(), &uri, None, Some(json!({"csr_pem": "x"})))
            .await;

        assert_eq!(status, StatusCode::OK, "{}", uri);
        assert_eq!(echoed["path"], resolve(backend, id), "{}", uri);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registry_health_reports_backend_state() {
    let harness = harness().await;

    let (status, body) = harness
        .send(Method::GET, "/api/registry/health", None, None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_token_endpoint_is_limited_to_private_networks() {
    let harness = harness().await;
    let uri = "/api/registry/internal/bootstrap-token";

    for peer in ["127.0.0.1:1", "10.1.2.3:1", "172.16.0.9:1", "192.168.5.5:1"] {
        let (status, echoed) = harness.send_from(peer, Method::GET, uri, None, None).await;
        assert_eq!(status, StatusCode::OK, "{}", peer);
        assert_eq!(echoed["path"], "/admin/bootstrap-tokens");
        assert_eq!(echoed["body"]["role"], "master");
        assert_eq!(echoed["body"]["max_uses"], 1);
    }

    for peer in ["8.8.8.8:1", "172.32.0.1:1", "[2001:db8::1]:1"] {
        let (status, _) = harness.send_from(peer, Method::GET, uri, None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{}", peer);
    }
}
