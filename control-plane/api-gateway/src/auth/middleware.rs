use crate::{
    auth::jwt::{verify_jwt, Claims},
    auth_service::SESSION_CUTOFF_PREFIX,
    AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, Method, StatusCode},
};
use chrono::Utc;
use entity::{InvalidJwt, User};
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, QueryFilter};

const COOKIE_NAME: &str = "auth_token";
const CSRF_HEADER: &str = "x-csfx-csrf";

pub fn request_token(headers: &HeaderMap) -> Option<&str> {
    bearer_token(headers).or_else(|| cookie_token(headers))
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
}

fn cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("cookie")
        .and_then(|cookie_header| cookie_header.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .filter_map(|cookie| cookie.trim().split_once('='))
                .find_map(|(name, value)| (name == COOKIE_NAME).then_some(value))
        })
}

fn csrf_violation(parts: &Parts) -> bool {
    let cookie_only = bearer_token(&parts.headers).is_none();
    let safe_method = matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS);
    cookie_only && !safe_method && !parts.headers.contains_key(CSRF_HEADER)
}

pub async fn is_token_revoked(db: &DatabaseConnection, claims: &Claims) -> Result<bool, DbErr> {
    let jti = claims.jti.to_string();
    let cutoff_prefix = format!("{SESSION_CUTOFF_PREFIX}{}:", claims.user_id);
    let rows = InvalidJwt::find()
        .filter(
            Condition::any()
                .add(entity::invalid_jwt::Column::Token.eq(jti.as_str()))
                .add(entity::invalid_jwt::Column::Token.starts_with(&cutoff_prefix)),
        )
        .all(db)
        .await?;
    Ok(rows.iter().any(|row| {
        row.token == jti
            || row
                .token
                .strip_prefix(&cutoff_prefix)
                .and_then(|cutoff| cutoff.parse::<i64>().ok())
                .is_some_and(|cutoff| claims.iat < cutoff)
    }))
}

pub async fn authenticate(parts: &Parts, state: &AppState) -> Result<Claims, StatusCode> {
    let token = request_token(&parts.headers).ok_or(StatusCode::UNAUTHORIZED)?;
    if csrf_violation(parts) {
        return Err(StatusCode::FORBIDDEN);
    }
    let claims = verify_jwt(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?
        .claims;
    if claims.exp < Utc::now().timestamp() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let internal = |_| StatusCode::INTERNAL_SERVER_ERROR;
    if is_token_revoked(&state.db_conn, &claims)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let user = User::find_by_id(claims.user_id)
        .one(&state.db_conn)
        .await
        .map_err(internal)?;
    user.map(|_| claims).ok_or(StatusCode::UNAUTHORIZED)
}

pub struct AuthenticatedUser(pub Claims);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        authenticate(parts, state).await.map(Self)
    }
}
