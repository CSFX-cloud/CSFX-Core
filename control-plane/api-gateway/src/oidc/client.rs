use std::{sync::OnceLock, time::Duration};

use entity::identity_providers;
use jsonwebtoken::{
    decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Header, Validation,
};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::error::OidcError;

const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

const ALLOWED_ALGORITHMS: [Algorithm; 9] = [
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
    Algorithm::HS256,
];

#[derive(Debug, Clone, Deserialize)]
pub struct Endpoints {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .expect("failed to build oidc http client")
    })
}

pub async fn discover(issuer_url: &str) -> Result<Endpoints, OidcError> {
    let base = issuer_url.trim_end_matches('/');
    let endpoints: Endpoints = http()
        .get(format!("{base}/.well-known/openid-configuration"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if endpoints.issuer.trim_end_matches('/') != base {
        return Err(OidcError::InvalidProvider("issuer mismatch"));
    }
    Ok(endpoints)
}

pub fn authorization_url(
    endpoints: &Endpoints,
    provider: &identity_providers::Model,
    redirect_uri: &str,
    state: &str,
    nonce: &str,
    code_challenge: &str,
) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", provider.client_id.as_str()),
        ("redirect_uri", redirect_uri),
        ("scope", provider.scopes.as_str()),
        ("state", state),
        ("nonce", nonce),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{key}={}", utf8_percent_encode(value, NON_ALPHANUMERIC)))
        .collect::<Vec<_>>()
        .join("&");
    let separator = if endpoints.authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };
    format!("{}{separator}{query}", endpoints.authorization_endpoint)
}

pub async fn exchange_code(
    endpoints: &Endpoints,
    provider: &identity_providers::Model,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> Result<String, OidcError> {
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", provider.client_id.as_str()),
        ("client_secret", client_secret),
        ("code_verifier", code_verifier),
    ];
    let response: TokenResponse = http()
        .post(&endpoints.token_endpoint)
        .form(&form)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(response.id_token)
}

async fn decoding_key(
    endpoints: &Endpoints,
    header: &Header,
    client_secret: &str,
) -> Result<DecodingKey, OidcError> {
    if header.alg == Algorithm::HS256 {
        return Ok(DecodingKey::from_secret(client_secret.as_bytes()));
    }
    let jwks: JwkSet = http()
        .get(&endpoints.jwks_uri)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let jwk = match header.kid.as_deref() {
        Some(kid) => jwks.find(kid),
        None => jwks.keys.first(),
    }
    .ok_or(OidcError::InvalidProvider("signing key not found"))?;
    Ok(DecodingKey::from_jwk(jwk)?)
}

pub async fn verify_id_token(
    endpoints: &Endpoints,
    provider: &identity_providers::Model,
    client_secret: &str,
    id_token: &str,
    expected_nonce: &str,
) -> Result<Map<String, Value>, OidcError> {
    let header = decode_header(id_token)?;
    if !ALLOWED_ALGORITHMS.contains(&header.alg) {
        return Err(OidcError::InvalidProvider("unsupported signing algorithm"));
    }
    let key = decoding_key(endpoints, &header, client_secret).await?;
    let mut validation = Validation::new(header.alg);
    validation.set_audience(&[provider.client_id.as_str()]);
    validation.set_issuer(&[endpoints.issuer.as_str()]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    let claims = decode::<Map<String, Value>>(id_token, &key, &validation)?.claims;
    if claims.get("nonce").and_then(Value::as_str) != Some(expected_nonce) {
        return Err(OidcError::AccessDenied("nonce_mismatch"));
    }
    Ok(claims)
}
