mod gauges;
mod http_metrics;
mod request_id;
mod server;

use axum::{middleware::from_fn, Router};

pub use gauges::{set_agent_heartbeat_lag_seconds, spawn_pool_gauges};
pub use request_id::{current_request_id, REQUEST_ID_HEADER};
pub use server::{metrics_router, render, serve_metrics};

pub fn instrument<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    http_metrics::register();
    router
        .layer(from_fn(http_metrics::record))
        .layer(from_fn(request_id::propagate))
}

#[cfg(test)]
mod tests;
