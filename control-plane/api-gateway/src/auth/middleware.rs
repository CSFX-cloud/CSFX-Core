use crate::{
    auth::jwt::{verify_jwt, Claims},
    auth_service::SESSION_CUTOFF_PREFIX,
    AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, StatusCode},
};
use chrono::Utc;
use entity::InvalidJwt;
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, QueryFilter};

pub fn request_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .or_else(|| {
            headers
                .get("cookie")
                .and_then(|cookie_header| cookie_header.to_str().ok())
                .and_then(|cookies| {
                    cookies.split(';').find_map(|cookie| {
                        let mut parts = cookie.trim().splitn(2, '=');
                        if parts.next()? == "token" {
                            parts.next()
                        } else {
                            None
                        }
                    })
                })
        })
}

pub async fn is_token_revoked(
    db: &DatabaseConnection,
    token: &str,
    claims: &Claims,
) -> Result<bool, DbErr> {
    let cutoff_prefix = format!("{SESSION_CUTOFF_PREFIX}{}:", claims.user_id);
    let rows = InvalidJwt::find()
        .filter(
            Condition::any()
                .add(entity::invalid_jwt::Column::Token.eq(token))
                .add(entity::invalid_jwt::Column::Token.starts_with(&cutoff_prefix)),
        )
        .all(db)
        .await?;
    Ok(rows.iter().any(|row| {
        row.token == token
            || row
                .token
                .strip_prefix(&cutoff_prefix)
                .and_then(|cutoff| cutoff.parse::<i64>().ok())
                .is_some_and(|cutoff| claims.iat < cutoff)
    }))
}

// Custom extractor for authenticated requests
pub struct AuthenticatedUser(pub Claims);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = request_token(&parts.headers);

        if let Some(token) = token {
            let claims = verify_jwt(token)
                .map_err(|_| StatusCode::UNAUTHORIZED)?
                .claims;
            if claims.exp < Utc::now().timestamp() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            let revoked = is_token_revoked(&state.db_conn, token, &claims)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if revoked {
                return Err(StatusCode::UNAUTHORIZED);
            }
            Ok(AuthenticatedUser(claims))
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
