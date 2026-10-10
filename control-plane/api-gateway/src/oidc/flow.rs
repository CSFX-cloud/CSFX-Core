use chrono::{Duration, Utc};
use entity::{
    identity_providers, oidc_exchange_codes, oidc_login_states, user, OidcExchangeCodes,
    OidcLoginStates, User,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

use super::{
    client, error::OidcError, identity::extract_identity, provisioning, random_token, redirect_uri,
    secrets, sha256_base64url,
};

const LOGIN_STATE_TTL_MINUTES: i64 = 10;
const EXCHANGE_CODE_TTL_SECONDS: i64 = 60;

pub async fn begin_login(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
) -> Result<String, OidcError> {
    let endpoints = client::discover(&provider.issuer_url).await?;
    let (state, nonce, verifier) = (random_token(), random_token(), random_token());
    let now = Utc::now().naive_utc();
    OidcLoginStates::delete_many()
        .filter(oidc_login_states::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    oidc_login_states::ActiveModel {
        state: Set(state.clone()),
        provider_id: Set(provider.id),
        nonce: Set(nonce.clone()),
        pkce_verifier: Set(verifier.clone()),
        expires_at: Set(now + Duration::minutes(LOGIN_STATE_TTL_MINUTES)),
    }
    .insert(db)
    .await?;
    Ok(client::authorization_url(
        &endpoints,
        provider,
        &redirect_uri(&provider.slug),
        &state,
        &nonce,
        &sha256_base64url(&verifier),
    ))
}

async fn consume_state(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
    state: &str,
) -> Result<oidc_login_states::Model, OidcError> {
    let stored = OidcLoginStates::find_by_id(state)
        .one(db)
        .await?
        .ok_or(OidcError::InvalidState)?;
    let deleted = OidcLoginStates::delete_by_id(state).exec(db).await?;
    let valid = deleted.rows_affected == 1
        && stored.provider_id == provider.id
        && stored.expires_at > Utc::now().naive_utc();
    valid.then_some(stored).ok_or(OidcError::InvalidState)
}

pub async fn complete_login(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
    code: &str,
    state: &str,
) -> Result<user::Model, OidcError> {
    let stored = consume_state(db, provider, state).await?;
    let endpoints = client::discover(&provider.issuer_url).await?;
    let client_secret = secrets::decrypt(&provider.client_secret_encrypted)?;
    let id_token = client::exchange_code(
        &endpoints,
        provider,
        &client_secret,
        &redirect_uri(&provider.slug),
        code,
        &stored.pkce_verifier,
    )
    .await?;
    let claims = client::verify_id_token(
        &endpoints,
        provider,
        &client_secret,
        &id_token,
        &stored.nonce,
    )
    .await?;
    let identity = extract_identity(&claims, provider)?;
    provisioning::sign_in(db, provider, &identity).await
}

pub async fn issue_exchange_code(
    db: &DatabaseConnection,
    user_id: uuid::Uuid,
) -> Result<String, OidcError> {
    let code = random_token();
    let now = Utc::now().naive_utc();
    OidcExchangeCodes::delete_many()
        .filter(oidc_exchange_codes::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    oidc_exchange_codes::ActiveModel {
        code_hash: Set(sha256_base64url(&code)),
        user_id: Set(user_id),
        expires_at: Set(now + Duration::seconds(EXCHANGE_CODE_TTL_SECONDS)),
    }
    .insert(db)
    .await?;
    Ok(code)
}

pub async fn redeem_exchange_code(
    db: &DatabaseConnection,
    code: &str,
) -> Result<user::Model, OidcError> {
    let hash = sha256_base64url(code);
    let stored = OidcExchangeCodes::find_by_id(hash.as_str())
        .one(db)
        .await?
        .ok_or(OidcError::InvalidState)?;
    let deleted = OidcExchangeCodes::delete_by_id(hash.as_str())
        .exec(db)
        .await?;
    if deleted.rows_affected != 1 || stored.expires_at <= Utc::now().naive_utc() {
        return Err(OidcError::InvalidState);
    }
    User::find_by_id(stored.user_id)
        .one(db)
        .await?
        .ok_or(OidcError::InvalidState)
}
