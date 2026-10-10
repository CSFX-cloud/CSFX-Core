use super::AppState;
use crate::auth::rate_limit::JwtOrIpKeyExtractor;
use crate::metrics;
use crate::utils::router_ext::RouterExt;
use axum::body::Body;
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum::http::Method;
use axum::http::{HeaderValue, Request, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use std::time::Duration;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::PeerIpKeyExtractor, GovernorLayer,
};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::{info_span, Span};

pub mod agent_proxy;
pub mod agent_stream;
pub mod agents;
pub mod buckets;
pub mod events;
pub mod identity_providers;
pub mod logs;
pub mod networks;
pub mod organizations;
pub mod registry;
pub mod releases;
pub mod resource_groups;
pub mod roles;
pub mod s3_proxy;
pub mod settings;
pub mod ssh_keys;
pub mod sso;
pub mod system;
pub mod update;
pub mod users;
pub mod volumes;
pub mod workloads;

fn replenish_period(requests_per_second: u64) -> Duration {
    Duration::from_secs_f64(1.0 / requests_per_second.max(1) as f64)
}

/// Creates the main application router and logs all registered routes.
pub fn create_router() -> Router<AppState> {
    let rate_limit_per_second: u64 = std::env::var("RATE_LIMIT_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    let burst_size: u32 = std::env::var("RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(JwtOrIpKeyExtractor::new())
            .period(replenish_period(rate_limit_per_second))
            .burst_size(burst_size)
            .finish()
            .expect("invalid rate limit configuration"),
    );

    let login_rate_limit_per_second: u64 = std::env::var("LOGIN_RATE_LIMIT_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let login_burst_size: u32 = std::env::var("LOGIN_RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let login_governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(PeerIpKeyExtractor)
            .period(replenish_period(login_rate_limit_per_second))
            .burst_size(login_burst_size)
            .finish()
            .expect("invalid login rate limit configuration"),
    );

    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    tracing::info!("CORS configured for frontend URL: {}", frontend_url);

    let allowed_origins = vec![
        "http://localhost:3000",
        "http://localhost:5173",
        "http://localhost:8000",
        "http://127.0.0.1:3000",
        "http://127.0.0.1:5173",
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
        .merge(registry::registry_routes())
        .merge(ssh_keys::ssh_keys_internal_routes())
        .merge(agent_stream::agent_stream_internal_routes());

    let agent_unmetered_router = Router::new()
        .merge(agents::agents_unmetered_routes())
        .merge(resource_groups::resource_groups_agent_routes())
        .merge(agent_stream::agent_stream_routes());

    let rate_limited_router = Router::new()
        .merge(agent_proxy::agent_proxy_routes())
        .merge(agents::agents_routes())
        .merge(buckets::buckets_routes())
        .merge(identity_providers::identity_provider_routes())
        .merge(roles::roles_routes())
        .merge(networks::networks_routes())
        .merge(organizations::routes())
        .merge(ssh_keys::ssh_keys_routes())
        .merge(system::routes())
        .merge(users::users_routes())
        .merge(volumes::volumes_routes())
        .merge(workloads::workloads_routes())
        .merge(events::events_routes())
        .merge(resource_groups::resource_groups_routes())
        .merge(logs::logs_routes())
        .merge(settings::settings_routes())
        .merge(s3_proxy::s3_proxy_routes())
        .layer(GovernorLayer::new(governor_config));

    let login_rate_limited_router = Router::new()
        .merge(users::public_users_routes())
        .merge(sso::sso_routes())
        .layer(GovernorLayer::new(login_governor_config));

    let api_router = Router::new()
        .merge(agent_unmetered_router)
        .merge(rate_limited_router)
        .merge(login_rate_limited_router)
        .merge(update::routes())
        .merge(releases::routes());

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "app/build".to_string());
    let index_path = format!("{}/index.html", static_dir);

    let serve_dir = ServeDir::new(&static_dir).not_found_service(ServeFile::new(&index_path));

    Router::new()
        .route("/metrics", get(metrics::metrics_handler))
        .merge(s3_proxy::object_data_router())
        .logged_nest("/api", api_router)
        .logged_nest("/api", internal_api_router)
        .fallback_service(serve_dir)
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
                    tracing::info!(target: "csfx::http_access", "started processing request")
                })
                .on_response(
                    |response: &Response<Body>, latency: std::time::Duration, _span: &Span| {
                        let status = response.status();
                        if status.is_server_error() {
                            tracing::error!(
                                target: "csfx::http_access",
                                status = status.as_u16(),
                                latency_ms = latency.as_millis(),
                                "request failed"
                            );
                        } else if status.is_client_error() {
                            tracing::warn!(
                                target: "csfx::http_access",
                                status = status.as_u16(),
                                latency_ms = latency.as_millis(),
                                "request rejected"
                            );
                        } else {
                            tracing::info!(
                                target: "csfx::http_access",
                                status = status.as_u16(),
                                latency_ms = latency.as_millis(),
                                "finished processing request"
                            );
                        }
                    },
                ),
        )
        .layer(cors)
}
