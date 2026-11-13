use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use prometheus::{register_counter_vec, register_histogram_vec, CounterVec, HistogramVec};
use std::sync::LazyLock;
use std::time::Instant;

const UNMATCHED_ROUTE: &str = "unmatched";
const LABELS: [&str; 3] = ["method", "path", "status"];

static REQUESTS_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!("csfx_http_requests_total", "Total HTTP requests", &LABELS)
        .expect("failed to register csfx_http_requests_total")
});

static REQUEST_DURATION_SECONDS: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "csfx_http_request_duration_seconds",
        "HTTP request duration in seconds",
        &LABELS
    )
    .expect("failed to register csfx_http_request_duration_seconds")
});

pub fn register() {
    LazyLock::force(&REQUESTS_TOTAL);
    LazyLock::force(&REQUEST_DURATION_SECONDS);
}

pub async fn record(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let matched = request.extensions().get::<MatchedPath>().cloned();
    let route = matched
        .as_ref()
        .map_or(UNMATCHED_ROUTE, MatchedPath::as_str);
    let started = Instant::now();

    let response = next.run(request).await;

    let status = response.status();
    let labels = [method.as_str(), route, status.as_str()];
    REQUESTS_TOTAL.with_label_values(&labels).inc();
    REQUEST_DURATION_SECONDS
        .with_label_values(&labels)
        .observe(started.elapsed().as_secs_f64());
    response
}
