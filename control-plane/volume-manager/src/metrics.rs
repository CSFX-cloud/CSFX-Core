use axum::response::IntoResponse;
use prometheus::{register_counter_vec, register_histogram_vec, CounterVec, Encoder, HistogramVec, TextEncoder};
use std::sync::OnceLock;

static HTTP_REQUESTS_TOTAL: OnceLock<CounterVec> = OnceLock::new();
static HTTP_REQUEST_DURATION_SECONDS: OnceLock<HistogramVec> = OnceLock::new();

pub fn init() {
    HTTP_REQUESTS_TOTAL.get_or_init(|| {
        register_counter_vec!(
            "csf_http_requests_total",
            "Total HTTP requests",
            &["method", "path", "status"]
        )
        .expect("failed to register csf_http_requests_total")
    });

    HTTP_REQUEST_DURATION_SECONDS.get_or_init(|| {
        register_histogram_vec!(
            "csf_http_request_duration_seconds",
            "HTTP request duration in seconds",
            &["method", "path"]
        )
        .expect("failed to register csf_http_request_duration_seconds")
    });
}

pub fn record_request(method: &str, path: &str, status: u16, duration_secs: f64) {
    if let Some(counter) = HTTP_REQUESTS_TOTAL.get() {
        counter
            .with_label_values(&[method, path, &status.to_string()])
            .inc();
    }
    if let Some(histogram) = HTTP_REQUEST_DURATION_SECONDS.get() {
        histogram
            .with_label_values(&[method, path])
            .observe(duration_secs);
    }
}

pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .expect("failed to encode metrics");
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        buffer,
    )
}
