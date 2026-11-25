use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use reqwest::RequestBuilder;
use ring::hmac;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const INTERNAL_TOKEN_HEADER: &str = "x-csfx-internal-token";
const HEALTH_PATH: &str = "/health";
const TOKEN_VERSION: &str = "v1";
const MAX_SKEW_SECONDS: u64 = 60;
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

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn caller_name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .map(|name| name.replace('.', "-"))
            .unwrap_or_else(|| "unknown".to_string())
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).ok())
        .collect()
}

fn payload(caller: &str, issued_at: u64) -> String {
    format!("{TOKEN_VERSION}.{caller}.{issued_at}")
}

fn sign(secret: &str, caller: &str, issued_at: u64) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let payload = payload(caller, issued_at);
    let tag = hmac::sign(&key, payload.as_bytes());
    format!("{payload}.{}", hex_encode(tag.as_ref()))
}

fn verified_caller(token: &str, secret: &str, now: u64) -> Option<String> {
    let mut parts = token.split('.');
    let (version, caller, issued_at, tag) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || version != TOKEN_VERSION {
        return None;
    }
    let issued_at: u64 = issued_at.parse().ok()?;
    if now.abs_diff(issued_at) > MAX_SKEW_SECONDS {
        return None;
    }
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    hmac::verify(
        &key,
        payload(caller, issued_at).as_bytes(),
        &hex_decode(tag)?,
    )
    .ok()?;
    Some(caller.to_string())
}

pub fn authorized_caller(headers: &HeaderMap) -> Option<String> {
    let secret = configured_token()?;
    let token = headers.get(INTERNAL_TOKEN_HEADER)?.to_str().ok()?;
    verified_caller(token, &secret, unix_now())
}

pub fn is_authorized(headers: &HeaderMap) -> bool {
    authorized_caller(headers).is_some()
}

pub fn signed_token() -> Option<String> {
    configured_token().map(|secret| sign(&secret, caller_name(), unix_now()))
}

pub fn token_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(mut value) = signed_token().and_then(|token| HeaderValue::from_str(&token).ok()) {
        value.set_sensitive(true);
        headers.insert(INTERNAL_TOKEN_HEADER, value);
    }
    headers
}

pub trait SignedRequest {
    fn signed(self) -> Self;
}

impl SignedRequest for RequestBuilder {
    fn signed(self) -> Self {
        self.headers(token_headers())
    }
}

pub async fn require_service_token(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if path == HEALTH_PATH || is_authorized(request.headers()) {
        return next.run(request).await;
    }
    tracing::warn!(path = %path, "rejected service request without valid service token");
    StatusCode::FORBIDDEN.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_tokens_verify_and_name_the_caller() {
        let token = sign("secret", "scheduler", 1000);
        assert_eq!(
            verified_caller(&token, "secret", 1010).as_deref(),
            Some("scheduler")
        );
    }

    #[test]
    fn tokens_are_rejected_when_wrong_stale_or_tampered() {
        let token = sign("secret", "scheduler", 1000);
        assert!(verified_caller(&token, "other", 1000).is_none());
        assert!(verified_caller(&token, "secret", 1000 + MAX_SKEW_SECONDS + 1).is_none());
        assert!(verified_caller(&token, "secret", 1000 - MAX_SKEW_SECONDS - 1).is_none());
        assert!(verified_caller(&token.replace("scheduler", "registry"), "secret", 1000).is_none());
        assert!(verified_caller("v1.scheduler.1000", "secret", 1000).is_none());
        assert!(verified_caller("secret", "secret", 1000).is_none());
        assert!(verified_caller(&format!("{token}.extra"), "secret", 1000).is_none());
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
        let valid = signed_token().unwrap();
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
            call("/data", Some("middleware-token"))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call("/data", Some(&valid)).await.unwrap().status(),
            StatusCode::OK
        );
    }
}
