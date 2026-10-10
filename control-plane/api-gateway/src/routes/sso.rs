use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Redirect},
    routing::{get, post},
    Router,
};
use entity::{identity_providers, IdentityProviders};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use super::users::AuthResponse;
use crate::{
    auth::jwt::create_jwt,
    oidc::{self, error::OidcError, flow},
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

fn login_failure_redirect(error: &OidcError) -> Redirect {
    Redirect::to(&format!(
        "{}/login?sso_error={}",
        oidc::frontend_url(),
        error.code()
    ))
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

async fn start(State(state): State<AppState>, Path(slug): Path<String>) -> Redirect {
    let result = async {
        let provider = find_enabled_provider(&state.db_conn, &slug).await?;
        flow::begin_login(&state.db_conn, &provider).await
    }
    .await;
    match result {
        Ok(url) => Redirect::to(&url),
        Err(error) => {
            tracing::warn!(error = %error, provider = %slug, "oidc login start failed");
            login_failure_redirect(&error)
        }
    }
}

async fn finish_callback(
    db: &DatabaseConnection,
    slug: &str,
    query: CallbackQuery,
) -> Result<String, OidcError> {
    if query.error.is_some() {
        return Err(OidcError::AccessDenied("provider_denied"));
    }
    let (Some(code), Some(login_state)) = (query.code, query.state) else {
        return Err(OidcError::InvalidState);
    };
    let provider = find_enabled_provider(db, slug).await?;
    let user = flow::complete_login(db, &provider, &code, &login_state).await?;
    flow::issue_exchange_code(db, user.id).await
}

async fn callback(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<CallbackQuery>,
) -> Redirect {
    match finish_callback(&state.db_conn, &slug, query).await {
        Ok(code) => Redirect::to(&format!(
            "{}/login/callback?code={}",
            oidc::frontend_url(),
            code
        )),
        Err(error) => {
            tracing::warn!(error = %error, provider = %slug, "oidc login failed");
            login_failure_redirect(&error)
        }
    }
}

async fn exchange(
    State(state): State<AppState>,
    Json(request): Json<ExchangeRequest>,
) -> Result<impl IntoResponse, StatusCode> {
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
    Ok(Json(AuthResponse {
        token,
        user_id: user.id.to_string(),
        username: user.name,
        two_factor_enabled: false,
        force_password_change: false,
    }))
}
