use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use reqwest::RequestBuilder;
use sha2::{Digest, Sha256};

pub const INTERNAL_TOKEN_HEADER: &str = "x-csfx-internal-token";
const HEALTH_PATH: &str = "/health";
pub const INTERNAL_TOKEN_ENV: &str = "CSFX_INTERNAL_TOKEN";
pub const GATEWAY_INTERNAL_ADDR_ENV: &str = "GATEWAY_INTERNAL_ADDR";
pub const GATEWAY_INTERNAL_URL_ENV: &str = "API_GATEWAY_INTERNAL_URL";
const DEFAULT_GATEWAY_INTERNAL_ADDR: &str = "127.0.0.1:8010";
const DEFAULT_GATEWAY_INTERNAL_URL: &str = "http://127.0.0.1:8010";

pub fn gateway_internal_addr() -> String {
    std::env::var(GATEWAY_INTERNAL_ADDR_ENV)
        .unwrap_or_else(|_| DEFAULT_GATEWAY_INTERNAL_ADDR.to_string())
}

pub fn gateway_internal_url() -> String {
    std::env::var(GATEWAY_INTERNAL_URL_ENV)
        .unwrap_or_else(|_| DEFAULT_GATEWAY_INTERNAL_URL.to_string())
}

pub fn configured_token() -> Option<String> {
    std::env::var(INTERNAL_TOKEN_ENV)
        .ok()
        .filter(|token| !token.is_empty())
}

fn matches(headers: &HeaderMap, expected: &str) -> bool {
    headers.get(INTERNAL_TOKEN_HEADER).is_some_and(|value| {
        Sha256::digest(value.as_bytes()) == Sha256::digest(expected.as_bytes())
    })
}

pub fn is_authorized(headers: &HeaderMap) -> bool {
    configured_token().is_some_and(|expected| matches(headers, &expected))
}

pub fn token_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(mut value) = configured_token().and_then(|token| HeaderValue::from_str(&token).ok())
    {
        value.set_sensitive(true);
        headers.insert(INTERNAL_TOKEN_HEADER, value);
    }
    headers
}

pub async fn require_service_token(request: Request, next: Next) -> Response {
    if request.uri().path() == HEALTH_PATH || is_authorized(request.headers()) {
        return next.run(request).await;
    }
    tracing::warn!(path = %request.uri().path(), "rejected service request without valid service token");
    StatusCode::FORBIDDEN.into_response()
}

pub fn authenticate(request: RequestBuilder) -> RequestBuilder {
    match configured_token() {
        Some(token) => request.header(INTERNAL_TOKEN_HEADER, token),
        None => request,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(INTERNAL_TOKEN_HEADER, value.parse().unwrap());
        headers
    }

    #[test]
    fn only_the_configured_token_matches() {
        assert!(matches(&headers("secret"), "secret"));
        assert!(!matches(&headers("secre"), "secret"));
        assert!(!matches(&headers("other!"), "secret"));
        assert!(!matches(&HeaderMap::new(), "secret"));
    }

    #[tokio::test]
    async fn middleware_guards_everything_except_health() {
        use axum::{body::Body, middleware::from_fn, routing::get, Router};
        use tower::ServiceExt;

        unsafe { std::env::set_var(INTERNAL_TOKEN_ENV, "middleware-token") };
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/data", get(|| async { "data" }))
            .layer(from_fn(require_service_token));
        let call = |path: &str, token: Option<&str>| {
            let mut request = axum::http::Request::builder().uri(path);
            if let Some(token) = token {
                request = request.header(INTERNAL_TOKEN_HEADER, token);
            }
            app.clone().oneshot(request.body(Body::empty()).unwrap())
        };
        assert_eq!(
            call("/health", None).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            call("/data", None).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call("/data", Some("wrong")).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call("/data", Some("middleware-token"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
}
