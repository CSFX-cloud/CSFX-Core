use axum::http::HeaderMap;
use reqwest::RequestBuilder;
use sha2::{Digest, Sha256};

pub const INTERNAL_TOKEN_HEADER: &str = "x-csfx-internal-token";
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
}
