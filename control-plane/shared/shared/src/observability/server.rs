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

pub async fn serve_metrics(default_addr: &str) -> anyhow::Result<()> {
    let raw = std::env::var(METRICS_ADDR_VARIABLE).unwrap_or_else(|_| default_addr.to_string());
    let addr: SocketAddr = raw.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(addr = %addr, "metrics listening");
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, metrics_router()).await {
            tracing::error!(error = %error, "metrics server failed");
        }
    });
    Ok(())
}
