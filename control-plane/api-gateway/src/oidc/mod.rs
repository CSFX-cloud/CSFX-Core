pub mod client;
pub mod error;
pub mod flow;
pub mod identity;
pub mod provisioning;
pub mod secrets;

use axum::http::HeaderMap;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};

pub fn random_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

pub fn sha256_base64url(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}

pub fn frontend_url() -> String {
    std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn default_scheme() -> &'static str {
    match std::env::var("TLS_ENABLED").as_deref() {
        Ok("false") => "http",
        _ => "https",
    }
}

pub fn public_origin(headers: &HeaderMap) -> String {
    if let Ok(configured) = std::env::var("PUBLIC_URL") {
        return configured.trim_end_matches('/').to_string();
    }
    let host = header_value(headers, "x-forwarded-host").or_else(|| header_value(headers, "host"));
    let Some(host) = host else {
        return frontend_url();
    };
    let scheme =
        header_value(headers, "x-forwarded-proto").unwrap_or_else(|| default_scheme().to_string());
    format!("{scheme}://{host}")
}

pub fn redirect_uri(origin: &str, slug: &str) -> String {
    format!("{origin}/api/auth/oidc/{slug}/callback")
}
