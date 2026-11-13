use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::{current_request_id, instrument, metrics_router, render, REQUEST_ID_HEADER};

fn app() -> Router {
    instrument(Router::new().route(
        "/probe/{id}",
        get(|| async {
            current_request_id()
                .map(|id| id.to_str().unwrap().to_string())
                .unwrap_or_default()
        }),
    ))
}

async fn send(app: &Router, uri: &str, request_id: Option<&str>) -> (StatusCode, String, String) {
    let mut builder = Request::builder().uri(uri);
    if let Some(id) = request_id {
        builder = builder.header(&REQUEST_ID_HEADER, id);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let header = response
        .headers()
        .get(&REQUEST_ID_HEADER)
        .map(|value| value.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, header, String::from_utf8(body.to_vec()).unwrap())
}

fn sample(metric: &str, labels: &str) -> f64 {
    let rendered = String::from_utf8(render()).unwrap();
    let prefix = format!("{metric}{{{labels}}} ");
    rendered
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|value| value.parse().unwrap())
        .unwrap_or(0.0)
}

#[tokio::test]
async fn request_is_counted_by_matched_route_with_a_latency_sample() {
    let app = app();
    let counter_labels = r#"method="GET",path="/probe/{id}",status="200""#;
    let before = sample("csfx_http_requests_total", counter_labels);
    let count_before = sample("csfx_http_request_duration_seconds_count", counter_labels);

    send(&app, "/probe/one", None).await;
    send(&app, "/probe/two", None).await;

    assert_eq!(
        sample("csfx_http_requests_total", counter_labels) - before,
        2.0
    );
    assert_eq!(
        sample("csfx_http_request_duration_seconds_count", counter_labels) - count_before,
        2.0
    );
    assert!(sample("csfx_http_request_duration_seconds_sum", counter_labels) > 0.0);
}

#[tokio::test]
async fn unknown_paths_share_one_route_label() {
    let app = app();
    let labels = r#"method="GET",path="unmatched",status="404""#;
    let before = sample("csfx_http_requests_total", labels);

    let (status, _, _) = send(&app, "/missing/a", None).await;
    send(&app, "/missing/b", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(sample("csfx_http_requests_total", labels) - before, 2.0);
}

#[tokio::test]
async fn request_id_is_generated_and_visible_to_the_handler() {
    let (_, header, body) = send(&app(), "/probe/one", None).await;

    assert!(uuid::Uuid::parse_str(&header).is_ok());
    assert_eq!(body, header);
}

#[tokio::test]
async fn incoming_request_id_is_kept_and_echoed() {
    let (_, header, body) = send(&app(), "/probe/one", Some("caller-supplied-1")).await;

    assert_eq!(header, "caller-supplied-1");
    assert_eq!(body, "caller-supplied-1");
}

#[tokio::test]
async fn oversized_request_id_is_replaced() {
    let long = "a".repeat(200);
    let (_, header, _) = send(&app(), "/probe/one", Some(&long)).await;

    assert!(uuid::Uuid::parse_str(&header).is_ok());
}

#[tokio::test]
async fn metrics_router_serves_prometheus_text() {
    let _ = send(&app(), "/probe/one", None).await;
    let (status, _, body) = send(&metrics_router(), "/metrics", None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("csfx_http_requests_total"));
}
