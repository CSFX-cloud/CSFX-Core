pub mod client;
pub mod keys;
pub mod scope;

use crate::config::Config;
use crate::error::ServiceError;
use crate::token::deployment_token_valid;
use axum::{
    extract::{RawQuery, State},
    http::{header, HeaderMap},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use entity::{agent_api_keys, site_deployments, AgentApiKeys, SiteDeployments};
use jsonwebtoken::{Algorithm, Header};
use keys::RegistryKeys;
use ring::digest;
use scope::{Access, Scope};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

pub const SERVICE_NAME: &str = "csfx-site-registry";
const TOKEN_TTL_SECONDS: i64 = 900;
const DEPLOYMENT_PREFIX: &str = "d-";
const AGENT_USER: &str = "agent";

#[derive(Clone)]
pub struct RegistryState {
    pub db: DatabaseConnection,
    pub config: Arc<Config>,
    pub keys: Arc<RegistryKeys>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Principal {
    Deployment { site_id: Uuid },
    Agent,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AccessEntry {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub actions: Vec<String>,
}

#[derive(Serialize)]
struct Claims {
    iss: &'static str,
    sub: String,
    aud: &'static str,
    iat: i64,
    nbf: i64,
    exp: i64,
    jti: String,
    access: Vec<AccessEntry>,
}

#[derive(Serialize)]
struct TokenResponse {
    token: String,
    access_token: String,
    expires_in: i64,
    issued_at: String,
}

pub fn allowed_access(principal: &Principal, requested: &Scope) -> Option<Access> {
    if !scope::is_repository(requested) {
        return None;
    }
    match principal {
        Principal::Deployment { site_id } if requested.name == site_id.to_string() => {
            Some(Access::PullPush)
        }
        Principal::Deployment { .. } => None,
        Principal::Agent => Some(Access::Pull),
    }
}

pub fn access_entries(principal: &Principal, requested: &[Scope]) -> Vec<AccessEntry> {
    requested
        .iter()
        .filter_map(|scope| {
            let allowed = allowed_access(principal, scope)?;
            let actions = scope::granted_actions(scope, allowed);
            (!actions.is_empty()).then(|| AccessEntry {
                kind: scope.kind.clone(),
                name: scope.name.clone(),
                actions,
            })
        })
        .collect()
}

fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let encoded = value.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(STANDARD.decode(encoded).ok()?).ok()?;
    let (user, password) = decoded.split_once(':')?;
    Some((user.to_string(), password.to_string()))
}

fn hash_key(key: &str) -> String {
    digest::digest(&digest::SHA256, key.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn deployment_principal(
    state: &RegistryState,
    deployment_id: Uuid,
    password: &str,
) -> Result<Principal, ServiceError> {
    if !deployment_token_valid(&state.config.secret, deployment_id, password) {
        return Err(ServiceError::Unauthorized);
    }
    let deployment = SiteDeployments::find_by_id(deployment_id)
        .filter(site_deployments::Column::Status.eq("building"))
        .one(&state.db)
        .await?
        .ok_or(ServiceError::Unauthorized)?;
    Ok(Principal::Deployment {
        site_id: deployment.site_id,
    })
}

async fn agent_principal(state: &RegistryState, password: &str) -> Result<Principal, ServiceError> {
    AgentApiKeys::find()
        .filter(agent_api_keys::Column::KeyHash.eq(hash_key(password)))
        .filter(agent_api_keys::Column::IsActive.eq(true))
        .one(&state.db)
        .await?
        .map(|_| Principal::Agent)
        .ok_or(ServiceError::Unauthorized)
}

async fn authenticate(
    state: &RegistryState,
    headers: &HeaderMap,
) -> Result<Principal, ServiceError> {
    let (user, password) = basic_credentials(headers).ok_or(ServiceError::Unauthorized)?;
    if user == AGENT_USER {
        return agent_principal(state, &password).await;
    }
    let id = user
        .strip_prefix(DEPLOYMENT_PREFIX)
        .and_then(|raw| Uuid::parse_str(raw).ok())
        .ok_or(ServiceError::Unauthorized)?;
    deployment_principal(state, id, &password).await
}

pub(crate) fn sign(
    keys: &RegistryKeys,
    subject: String,
    access: Vec<AccessEntry>,
) -> Result<String, ServiceError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        iss: SERVICE_NAME,
        sub: subject,
        aud: SERVICE_NAME,
        iat: now,
        nbf: now,
        exp: now + TOKEN_TTL_SECONDS,
        jti: Uuid::new_v4().to_string(),
        access,
    };
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.key_id.clone());
    jsonwebtoken::encode(&header, &claims, &keys.encoding)
        .map_err(|error| ServiceError::Crypto(error.to_string()))
}

async fn token_handler(
    State(state): State<RegistryState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<Json<TokenResponse>, ServiceError> {
    let principal = authenticate(&state, &headers).await?;
    let query = query.unwrap_or_default();
    if scope::query_value(&query, "service").is_some_and(|service| service != SERVICE_NAME) {
        return Err(ServiceError::Unauthorized);
    }
    let access = access_entries(&principal, &scope::parse_query(&query));
    let subject = scope::query_value(&query, "account").unwrap_or_else(|| AGENT_USER.to_string());
    let token = sign(&state.keys, subject, access)?;
    Ok(Json(TokenResponse {
        access_token: token.clone(),
        token,
        expires_in: TOKEN_TTL_SECONDS,
        issued_at: Utc::now().to_rfc3339(),
    }))
}

async fn certificate_handler(State(state): State<RegistryState>) -> String {
    state.keys.certificate_pem.clone()
}

pub fn router(state: RegistryState) -> Router {
    Router::new()
        .route("/registry/token", get(token_handler))
        .route("/registry/certificate", get(certificate_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requested(raw: &str) -> Vec<Scope> {
        scope::parse_query(&format!("scope={raw}"))
    }

    #[test]
    fn deployments_may_only_touch_their_own_site_repository() {
        let site = Uuid::from_u128(7);
        let principal = Principal::Deployment { site_id: site };
        let own = access_entries(
            &principal,
            &requested(&format!("repository:{site}:pull,push")),
        );
        assert_eq!(own[0].actions, vec!["pull", "push"]);
        let other = access_entries(&principal, &requested("repository:someone-else:pull,push"));
        assert!(other.is_empty());
    }

    #[test]
    fn agents_can_only_pull() {
        let entries = access_entries(&Principal::Agent, &requested("repository:any:pull,push"));
        assert_eq!(entries[0].actions, vec!["pull"]);
        assert!(access_entries(&Principal::Agent, &requested("registry:catalog:*")).is_empty());
    }

    #[test]
    fn basic_credentials_are_decoded() {
        let mut headers = HeaderMap::new();
        let value = format!("Basic {}", STANDARD.encode("agent:secret:with:colons"));
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        assert_eq!(
            basic_credentials(&headers),
            Some(("agent".to_string(), "secret:with:colons".to_string()))
        );
        assert!(basic_credentials(&HeaderMap::new()).is_none());
    }

    #[test]
    fn signed_tokens_carry_the_rs256_header_and_claims() {
        let (key, certificate) = keys::generate().unwrap();
        let keys = RegistryKeys {
            encoding: jsonwebtoken::EncodingKey::from_rsa_pem(key.as_bytes()).unwrap(),
            key_id: "kid".to_string(),
            certificate_pem: certificate,
        };
        let access = access_entries(&Principal::Agent, &requested("repository:a:pull"));
        let token = sign(&keys, "agent".to_string(), access).unwrap();
        let header = jsonwebtoken::decode_header(&token).unwrap();
        assert_eq!(header.alg, Algorithm::RS256);
        assert_eq!(header.kid.as_deref(), Some("kid"));
        let payload = token.split('.').nth(1).unwrap();
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .unwrap();
        let claims: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(claims["access"][0]["name"], "a");
        assert_eq!(claims["aud"], SERVICE_NAME);
    }
}
