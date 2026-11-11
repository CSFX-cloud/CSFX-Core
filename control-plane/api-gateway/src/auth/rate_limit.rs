use axum::http::Request;
use sha2::{Digest, Sha256};
use tower_governor::key_extractor::{KeyExtractor, PeerIpKeyExtractor};
use tower_governor::GovernorError;
use uuid::Uuid;

use super::jwt::verify_jwt;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum RateLimitKey {
    User(Uuid),
    AgentApiKey([u8; 32]),
    Ip(std::net::IpAddr),
}

#[derive(Clone)]
pub struct JwtOrIpKeyExtractor {
    fallback: PeerIpKeyExtractor,
}

impl JwtOrIpKeyExtractor {
    pub fn new() -> Self {
        Self {
            fallback: PeerIpKeyExtractor,
        }
    }
}

impl Default for JwtOrIpKeyExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyExtractor for JwtOrIpKeyExtractor {
    type Key = RateLimitKey;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(user_id) = extract_user_id(req) {
            return Ok(RateLimitKey::User(user_id));
        }

        if let Some(key_hash) = extract_agent_api_key_hash(req) {
            return Ok(RateLimitKey::AgentApiKey(key_hash));
        }

        self.fallback.extract(req).map(RateLimitKey::Ip)
    }
}

fn extract_user_id<T>(req: &Request<T>) -> Option<Uuid> {
    let header = req.headers().get(axum::http::header::AUTHORIZATION)?;
    let value = header.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    let claims = verify_jwt(token).ok()?;
    Some(claims.claims.user_id)
}

fn extract_agent_api_key_hash<T>(req: &Request<T>) -> Option<[u8; 32]> {
    let header = req.headers().get("X-API-Key")?;
    let raw_key = header.to_str().ok()?;
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    Some(hasher.finalize().into())
}
