use axum::{
    body::Body,
    extract::Request,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use http::StatusCode;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[derive(Clone)]
pub struct FrontendState {
    client: Client,
    frontend_url: String,
}

impl FrontendState {
    pub fn new() -> Self {
        // Always read from environment variable, with fallback to localhost:3000
        let frontend_url =
            std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

        tracing::info!("Frontend proxy configured to: {}", frontend_url);

        Self {
            client: hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(HttpConnector::new()),
            frontend_url,
        }
    }
}

pub fn frontend_router() -> Router {
    let state = FrontendState::new();

    // Proxy root and all other routes to frontend
    Router::new()
        .route("/", get(handler))
        .route("/*path", get(handler))
        .with_state(state)
}

async fn handler(
    axum::extract::State(state): axum::extract::State<FrontendState>,
    mut req: Request,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Skip API routes - they should be handled by the API router
    if path.starts_with("/api") || path.starts_with("/swagger-ui") {
        return Err(StatusCode::NOT_FOUND);
    }

    let path_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(path);

    let uri = format!("{}{}", state.frontend_url, path_query);

    tracing::debug!("Proxying request to frontend: {} -> {}", path, uri);

    *req.uri_mut() = uri.parse().map_err(|e| {
        tracing::error!("Failed to parse URI: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    state
        .client
        .request(req)
        .await
        .map(|res| res.into_response())
        .map_err(|e| {
            tracing::error!("Frontend proxy error: {}", e);
            StatusCode::BAD_GATEWAY
        })
}
