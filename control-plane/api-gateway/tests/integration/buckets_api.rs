use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::common::harness;

struct Case {
    method: Method,
    uri: &'static str,
    backend: &'static str,
    body: Option<Value>,
}

fn case(method: Method, uri: &'static str, backend: &'static str, body: Option<Value>) -> Case {
    Case {
        method,
        uri,
        backend,
        body,
    }
}

fn cases() -> Vec<Case> {
    let payload = Some(json!({"name": "assets"}));
    vec![
        case(Method::POST, "/api/buckets", "/buckets", payload.clone()),
        case(Method::GET, "/api/buckets", "/buckets", None),
        case(Method::GET, "/api/buckets/00000000-0000-0000-0000-0000000000b1", "/buckets/00000000-0000-0000-0000-0000000000b1", None),
        case(
            Method::PATCH,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1",
            "/buckets/00000000-0000-0000-0000-0000000000b1",
            payload.clone(),
        ),
        case(Method::DELETE, "/api/buckets/00000000-0000-0000-0000-0000000000b1", "/buckets/00000000-0000-0000-0000-0000000000b1", None),
        case(
            Method::GET,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/keys",
            "/buckets/00000000-0000-0000-0000-0000000000b1/keys",
            None,
        ),
        case(
            Method::POST,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/keys",
            "/buckets/00000000-0000-0000-0000-0000000000b1/keys",
            payload.clone(),
        ),
        case(
            Method::POST,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/keys/00000000-0000-0000-0000-0000000000c1/rotate",
            "/buckets/00000000-0000-0000-0000-0000000000b1/keys/00000000-0000-0000-0000-0000000000c1/rotate",
            None,
        ),
        case(
            Method::DELETE,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/keys/00000000-0000-0000-0000-0000000000c1",
            "/buckets/00000000-0000-0000-0000-0000000000b1/keys/00000000-0000-0000-0000-0000000000c1",
            None,
        ),
        case(
            Method::GET,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/objects?prefix=a%20b",
            "/buckets/00000000-0000-0000-0000-0000000000b1/objects",
            None,
        ),
        case(
            Method::DELETE,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/objects/dir/file.txt",
            "/buckets/00000000-0000-0000-0000-0000000000b1/objects/dir/file.txt",
            None,
        ),
        case(
            Method::POST,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/objects/presign-upload",
            "/buckets/00000000-0000-0000-0000-0000000000b1/objects/presign-upload",
            payload,
        ),
        case(
            Method::GET,
            "/api/buckets/00000000-0000-0000-0000-0000000000b1/objects/presign-download/dir/file.txt",
            "/buckets/00000000-0000-0000-0000-0000000000b1/objects/presign-download/dir/file.txt",
            None,
        ),
        case(Method::GET, "/api/object-storage/cluster", "/cluster", None),
    ]
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bucket_requests_are_forwarded_to_object_storage() {
    let harness = harness().await;
    for case in cases() {
        let (status, echoed) = harness
            .admin(case.method.clone(), case.uri, case.body.clone())
            .await;
        assert_eq!(status, StatusCode::OK, "{}", case.uri);
        assert_eq!(echoed["path"], case.backend, "{}", case.uri);
        assert_eq!(echoed["method"], case.method.as_str(), "{}", case.uri);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn object_listing_encodes_prefix_and_continuation_token() {
    let harness = harness().await;
    let uri = "/api/buckets/00000000-0000-0000-0000-0000000000b1/objects?prefix=a%20b%2Fc&continuation_token=t%2B1";
    let (status, echoed) = harness.get(uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(echoed["query"], "prefix=a%20b%2Fc&continuation_token=t%2B1");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bucket_routes_require_authentication_and_permissions() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    for case in cases() {
        let anonymous = harness
            .send(case.method.clone(), case.uri, None, case.body.clone())
            .await;
        assert_eq!(anonymous.0, StatusCode::UNAUTHORIZED, "{}", case.uri);
        let denied = harness
            .send(
                case.method.clone(),
                case.uri,
                Some(&limited),
                case.body.clone(),
            )
            .await;
        assert_eq!(
            denied.0,
            StatusCode::FORBIDDEN,
            "{} {}",
            case.method,
            case.uri
        );
    }
}
