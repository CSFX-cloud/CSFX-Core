use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration as StdDuration;
use uuid::Uuid;

use crate::ttl_cache::TtlCache;

pub const TOKEN_LIFETIME_HOURS: i64 = 24;
const MIN_JWT_SECRET_BYTES: usize = 32;
const TICKET_LIFETIME_SECONDS: i64 = 30;
const TICKET_REPLAY_WINDOW: StdDuration = StdDuration::from_secs(120);

static REDEEMED_TICKETS: LazyLock<TtlCache<()>> =
    LazyLock::new(|| TtlCache::new(TICKET_REPLAY_WINDOW));

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
    pub jti: Uuid,
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
            jti: Uuid::new_v4(),
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

pub const EXEC_PURPOSE: &str = "workload-exec";
pub const VNC_PURPOSE: &str = "workload-vnc";
pub const NODE_METRICS_PURPOSE: &str = "node-metrics";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketClaims {
    pub jti: Uuid,
    pub subject: Uuid,
    pub user_id: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub purpose: String,
}

pub fn create_ticket(
    purpose: &str,
    subject: Uuid,
    user_id: Uuid,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = TicketClaims {
        jti: Uuid::new_v4(),
        subject,
        user_id,
        exp: (now + Duration::seconds(TICKET_LIFETIME_SECONDS)).timestamp(),
        iat: now.timestamp(),
        purpose: purpose.to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(get_jwt_secret().as_ref()),
    )
}

pub fn verify_ticket(
    token: &str,
    purpose: &str,
    subject: Uuid,
) -> Result<TicketClaims, jsonwebtoken::errors::Error> {
    let data = decode::<TicketClaims>(
        token,
        &DecodingKey::from_secret(get_jwt_secret().as_ref()),
        &Validation::default(),
    )?;

    if data.claims.purpose != purpose || data.claims.subject != subject {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    if !REDEEMED_TICKETS.insert_if_absent(&data.claims.jti.to_string(), ()) {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_can_be_redeemed_once() {
        std::env::set_var("JWT_SECRET", "t".repeat(MIN_JWT_SECRET_BYTES));
        let subject = Uuid::new_v4();
        let ticket = create_ticket(EXEC_PURPOSE, subject, Uuid::new_v4()).unwrap();
        assert!(verify_ticket(&ticket, EXEC_PURPOSE, subject).is_ok());
        assert!(verify_ticket(&ticket, EXEC_PURPOSE, subject).is_err());
    }

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
