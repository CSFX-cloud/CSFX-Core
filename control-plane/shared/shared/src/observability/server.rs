use axum::{http::header, response::IntoResponse, routing::get, Router};
use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;

const METRICS_ADDR_VARIABLE: &str = "METRICS_ADDR";

pub fn render() -> Vec<u8> {
    let mut buffer = Vec::new();
    TextEncoder::new()
        .encode(&prometheus::gather(), &mut buffer)
        .expect("failed to encode metrics");
    buffer
}

async fn metrics_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        render(),
    )
}

pub fn metrics_router() -> Router {
    Router::new().route("/metrics", get(metrics_handler))
}

async fn bind(default_addr: &str) -> anyhow::Result<tokio::net::TcpListener> {
    let raw = std::env::var(METRICS_ADDR_VARIABLE).unwrap_or_else(|_| default_addr.to_string());
    let addr: SocketAddr = raw.parse()?;
    Ok(tokio::net::TcpListener::bind(addr).await?)
}

pub async fn serve_metrics(default_addr: &str) {
    let listener = match bind(default_addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(error = %error, "metrics server disabled");
            return;
        }
    };
    tracing::info!(addr = ?listener.local_addr().ok(), "metrics listening");
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, metrics_router()).await {
            tracing::error!(error = %error, "metrics server failed");
        }
    });
}
