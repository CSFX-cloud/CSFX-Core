use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{AppendHeaders, IntoResponse, Json, Redirect},
    routing::{get, post},
    Router,
};
use entity::{identity_providers, IdentityProviders};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use super::users::AuthResponse;
use crate::{
    auth::jwt::create_jwt,
    oidc::{self, binding, error::OidcError, flow},
    AppState,
};

pub fn sso_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/providers", get(list_providers))
        .route("/auth/oidc/exchange", post(exchange))
        .route("/auth/oidc/{slug}/start", get(start))
        .route("/auth/oidc/{slug}/callback", get(callback))
}

#[derive(Serialize)]
struct PublicProvider {
    slug: String,
    display_name: String,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ExchangeRequest {
    code: String,
}

async fn find_enabled_provider(
    db: &DatabaseConnection,
    slug: &str,
) -> Result<identity_providers::Model, OidcError> {
    IdentityProviders::find()
        .filter(identity_providers::Column::Slug.eq(slug))
        .filter(identity_providers::Column::Enabled.eq(true))
        .one(db)
        .await?
        .ok_or(OidcError::AccessDenied("unknown_provider"))
}

fn login_failure_redirect(origin: &str, error: &OidcError) -> Redirect {
    Redirect::to(&format!("{origin}/login?sso_error={}", error.code()))
}

async fn list_providers(
    State(state): State<AppState>,
) -> Result<Json<Vec<PublicProvider>>, StatusCode> {
    let providers = IdentityProviders::find()
        .filter(identity_providers::Column::Enabled.eq(true))
        .order_by_asc(identity_providers::Column::DisplayName)
        .all(&state.db_conn)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "failed to list identity providers");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(
        providers
            .into_iter()
            .map(|provider| PublicProvider {
                slug: provider.slug,
                display_name: provider.display_name,
            })
            .collect(),
    ))
}

fn with_cookies(redirect: Redirect, cookies: Vec<String>) -> axum::response::Response {
    let cookies: Vec<(header::HeaderName, HeaderValue)> = cookies
        .iter()
        .filter_map(|cookie| HeaderValue::from_str(cookie).ok())
        .map(|value| (header::SET_COOKIE, value))
        .collect();
    (AppendHeaders(cookies), redirect).into_response()
}

async fn start(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> axum::response::Response {
    let origin = oidc::public_origin(&headers);
    let result = async {
        let provider = find_enabled_provider(&state.db_conn, &slug).await?;
        let redirect_uri = oidc::redirect_uri(&origin, &slug);
        flow::begin_login(&state.db_conn, &provider, &redirect_uri).await
    }
    .await;
    match result {
        Ok(login) => with_cookies(
            Redirect::to(&login.authorization_url),
            vec![binding::bind_state(&origin, &login.state)],
        ),
        Err(error) => {
            tracing::warn!(error = %error, provider = %slug, "oidc login start failed");
            login_failure_redirect(&origin, &error).into_response()
        }
    }
}

async fn finish_callback(
    db: &DatabaseConnection,
    slug: &str,
    redirect_uri: &str,
    query: CallbackQuery,
    headers: &HeaderMap,
) -> Result<String, OidcError> {
    if query.error.is_some() {
        return Err(OidcError::AccessDenied("provider_denied"));
    }
    let (Some(code), Some(login_state)) = (query.code, query.state) else {
        return Err(OidcError::InvalidState);
    };
    if !binding::state_is_bound(headers, &login_state) {
        return Err(OidcError::InvalidState);
    }
    let provider = find_enabled_provider(db, slug).await?;
    let user = flow::complete_login(db, &provider, redirect_uri, &code, &login_state).await?;
    flow::issue_exchange_code(db, user.id).await
}

async fn callback(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> axum::response::Response {
    let origin = oidc::public_origin(&headers);
    let redirect_uri = oidc::redirect_uri(&origin, &slug);
    let outcome = finish_callback(&state.db_conn, &slug, &redirect_uri, query, &headers).await;
    let clear_state = binding::clear(&origin, binding::STATE_COOKIE);
    match outcome {
        Ok(code) => with_cookies(
            Redirect::to(&format!("{origin}/login/callback?code={code}")),
            vec![clear_state, binding::bind_exchange_code(&origin, &code)],
        ),
        Err(error) => {
            tracing::warn!(error = %error, provider = %slug, "oidc login failed");
            with_cookies(login_failure_redirect(&origin, &error), vec![clear_state])
        }
    }
}

async fn exchange(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ExchangeRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if !binding::exchange_code_is_bound(&headers, &request.code) {
        tracing::warn!("oidc exchange code presented without its browser binding");
        return Err(StatusCode::UNAUTHORIZED);
    }
    let origin = oidc::public_origin(&headers);
    let user = flow::redeem_exchange_code(&state.db_conn, &request.code)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "oidc exchange code rejected");
            StatusCode::UNAUTHORIZED
        })?;
    let token = create_jwt(user.id, user.name.clone()).map_err(|error| {
        tracing::error!(error = %error, "failed to issue jwt");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let clear_exchange = binding::clear(&origin, binding::EXCHANGE_COOKIE);
    Ok((
        AppendHeaders([(header::SET_COOKIE, clear_exchange)]),
        Json(AuthResponse {
            token,
            user_id: user.id.to_string(),
            username: user.name,
            two_factor_enabled: false,
            force_password_change: false,
        }),
    ))
}
