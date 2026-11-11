use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::OnceLock;
use uuid::Uuid;

pub const TOKEN_LIFETIME_HOURS: i64 = 24;
const MIN_JWT_SECRET_BYTES: usize = 32;

static JWT_SECRET: OnceLock<String> = OnceLock::new();

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum JwtSecretError {
    #[error("environment variable JWT_SECRET is not set")]
    Missing,
    #[error("environment variable JWT_SECRET must be at least {MIN_JWT_SECRET_BYTES} bytes long")]
    TooShort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub exp: i64,    // Expiration time
    pub iat: i64,    // Issued at
    pub user_id: Uuid,
    pub username: String,
}

impl Claims {
    pub fn new(user_id: Uuid, username: String) -> Self {
        let now = Utc::now();
        let exp = now + Duration::hours(TOKEN_LIFETIME_HOURS);

        Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            user_id,
            username,
        }
    }
}

pub fn create_jwt(user_id: Uuid, username: String) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims::new(user_id, username);
    let secret = get_jwt_secret();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

pub fn verify_jwt(token: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    let secret = get_jwt_secret();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
}

fn parse_jwt_secret(value: Option<String>) -> Result<String, JwtSecretError> {
    let secret = value.ok_or(JwtSecretError::Missing)?;
    if secret.len() < MIN_JWT_SECRET_BYTES {
        return Err(JwtSecretError::TooShort);
    }
    Ok(secret)
}

pub fn init_jwt_secret() -> Result<(), JwtSecretError> {
    let secret = parse_jwt_secret(env::var("JWT_SECRET").ok())?;
    let _ = JWT_SECRET.set(secret);
    Ok(())
}

pub fn get_jwt_secret() -> &'static str {
    JWT_SECRET.get_or_init(|| {
        parse_jwt_secret(env::var("JWT_SECRET").ok()).expect("JWT_SECRET is missing or too short")
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecTicketClaims {
    pub workload_id: Uuid,
    pub user_id: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub purpose: String,
}

impl ExecTicketClaims {
    pub fn new(workload_id: Uuid, user_id: Uuid) -> Self {
        let now = Utc::now();
        ExecTicketClaims {
            workload_id,
            user_id,
            exp: (now + Duration::seconds(30)).timestamp(),
            iat: now.timestamp(),
            purpose: "workload-exec".to_string(),
        }
    }
}

pub fn create_exec_ticket(
    workload_id: Uuid,
    user_id: Uuid,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = ExecTicketClaims::new(workload_id, user_id);
    let secret = get_jwt_secret();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

pub fn verify_exec_ticket(
    token: &str,
    workload_id: Uuid,
) -> Result<ExecTicketClaims, jsonwebtoken::errors::Error> {
    let secret = get_jwt_secret();
    let data = decode::<ExecTicketClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;

    if data.claims.purpose != "workload-exec" || data.claims.workload_id != workload_id {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    Ok(data.claims)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VncTicketClaims {
    pub workload_id: Uuid,
    pub user_id: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub purpose: String,
}

impl VncTicketClaims {
    pub fn new(workload_id: Uuid, user_id: Uuid) -> Self {
        let now = Utc::now();
        VncTicketClaims {
            workload_id,
            user_id,
            exp: (now + Duration::seconds(30)).timestamp(),
            iat: now.timestamp(),
            purpose: "workload-vnc".to_string(),
        }
    }
}

pub fn create_vnc_ticket(
    workload_id: Uuid,
    user_id: Uuid,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = VncTicketClaims::new(workload_id, user_id);
    let secret = get_jwt_secret();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

pub fn verify_vnc_ticket(
    token: &str,
    workload_id: Uuid,
) -> Result<VncTicketClaims, jsonwebtoken::errors::Error> {
    let secret = get_jwt_secret();
    let data = decode::<VncTicketClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;

    if data.claims.purpose != "workload-vnc" || data.claims.workload_id != workload_id {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    Ok(data.claims)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetricsTicketClaims {
    pub agent_id: Uuid,
    pub user_id: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub purpose: String,
}

impl NodeMetricsTicketClaims {
    pub fn new(agent_id: Uuid, user_id: Uuid) -> Self {
        let now = Utc::now();
        NodeMetricsTicketClaims {
            agent_id,
            user_id,
            exp: (now + Duration::seconds(30)).timestamp(),
            iat: now.timestamp(),
            purpose: "node-metrics".to_string(),
        }
    }
}

pub fn create_node_metrics_ticket(
    agent_id: Uuid,
    user_id: Uuid,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = NodeMetricsTicketClaims::new(agent_id, user_id);
    let secret = get_jwt_secret();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

pub fn verify_node_metrics_ticket(
    token: &str,
    agent_id: Uuid,
) -> Result<NodeMetricsTicketClaims, jsonwebtoken::errors::Error> {
    let secret = get_jwt_secret();
    let data = decode::<NodeMetricsTicketClaims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;

    if data.claims.purpose != "node-metrics" || data.claims.agent_id != agent_id {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_secret_is_rejected() {
        assert_eq!(parse_jwt_secret(None), Err(JwtSecretError::Missing));
    }

    #[test]
    fn short_secret_is_rejected() {
        let short = "a".repeat(MIN_JWT_SECRET_BYTES - 1);
        assert_eq!(parse_jwt_secret(Some(short)), Err(JwtSecretError::TooShort));
    }

    #[test]
    fn secret_of_minimum_length_is_accepted() {
        let secret = "a".repeat(MIN_JWT_SECRET_BYTES);
        assert_eq!(parse_jwt_secret(Some(secret.clone())), Ok(secret));
    }
}
