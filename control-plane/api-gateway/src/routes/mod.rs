use super::AppState;
use crate::metrics;
use crate::utils::router_ext::RouterExt;
use axum::body::Body;
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum::http::Method;
use axum::http::{HeaderValue, Request, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info_span, Span};

pub mod agents;
pub mod events;
pub mod networks;
pub mod organizations;
pub mod registry;
pub mod system;
pub mod update;
pub mod users;
pub mod volumes;
pub mod workloads;

/// Creates the main application router and logs all registered routes.
pub fn create_router() -> Router<AppState> {
    let rate_limit_per_second: u64 = std::env::var("RATE_LIMIT_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let burst_size: u32 = std::env::var("RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(rate_limit_per_second)
            .burst_size(burst_size)
            .finish()
            .expect("invalid rate limit configuration"),
    );

    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    tracing::info!("CORS configured for frontend URL: {}", frontend_url);

    // Allow multiple origins for development (both Docker container and localhost)
    let allowed_origins = vec![
        "http://localhost:3000",
        "http://localhost:8000",
        "http://127.0.0.1:3000",
        "http://127.0.0.1:8000",
        &frontend_url,
    ];

    let cors = CorsLayer::new()
        .allow_origin(
            allowed_origins
                .into_iter()
                .filter_map(|origin| origin.parse::<HeaderValue>().ok())
                .collect::<Vec<_>>(),
        )
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(vec![AUTHORIZATION, ACCEPT, CONTENT_TYPE])
        .allow_credentials(true);

    let internal_api_router = Router::new()
        .merge(registry::registry_routes());

    let api_router = Router::new()
        .merge(agents::agents_routes())
        .merge(networks::networks_routes())
        .merge(organizations::routes())
        .merge(system::routes())
        .merge(update::routes())
        .merge(users::users_routes())
        .merge(volumes::volumes_routes())
        .merge(workloads::workloads_routes())
        .merge(events::events_routes())
        .layer(GovernorLayer {
            config: governor_config,
        });

    Router::new()
        .route("/metrics", get(metrics::metrics_handler))
        .logged_nest("/api", api_router)
        .logged_nest("/api", internal_api_router)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        version = ?request.version(),
                    )
                })
                .on_request(|_request: &Request<Body>, _span: &Span| {
                    tracing::info!("started processing request")
                })
                .on_response(
                    |_response: &Response<Body>, latency: std::time::Duration, _span: &Span| {
                        tracing::info!(
                            latency_ms = latency.as_millis(),
                            "finished processing request"
                        )
                    },
                ),
        )
        .layer(cors)
}
