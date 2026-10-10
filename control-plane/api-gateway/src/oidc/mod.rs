pub mod client;
pub mod error;
pub mod flow;
pub mod identity;
pub mod provisioning;
pub mod secrets;

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

pub fn redirect_uri(slug: &str) -> String {
    format!("{}/api/auth/oidc/{}/callback", frontend_url(), slug)
}
