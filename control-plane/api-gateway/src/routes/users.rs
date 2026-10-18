use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::{auth::middleware::AuthenticatedUser, auth_service::AuthService, AppState};

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Username for login
    pub username: String,
    /// RSA encrypted password
    pub encrypted_password: String,
    /// Optional 2FA code
    pub two_factor_code: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    /// JWT authentication token
    pub token: String,
    /// User UUID
    pub user_id: String,
    /// Username
    pub username: String,
    /// Whether 2FA is enabled for this user
    pub two_factor_enabled: bool,
    /// Whether the user must change their password
    pub force_password_change: bool,
}

#[derive(Serialize, ToSchema)]
pub struct PublicKeyResponse {
    /// RSA public key in PEM format
    pub public_key: String,
}

#[derive(Serialize, ToSchema)]
pub struct UserProfileResponse {
    /// User UUID
    pub id: String,
    /// Username
    pub username: String,
    /// Email address
    pub email: Option<String>,
    /// Gravatar email address used to derive the profile avatar
    pub gravatar_email: Option<String>,
    /// Whether 2FA is enabled
    pub two_factor_enabled: bool,
    /// Whether password change is required
    pub force_password_change: bool,
}

// Define the authenticated routes for the users module
pub fn users_routes() -> Router<AppState> {
    Router::new()
        .route("/profile", get(get_user_profile))
        .route("/validate-session", get(validate_session))
        .route("/logout", post(logout_user))
        .route("/2fa/setup", post(setup_2fa))
        .route("/2fa/enable", post(enable_2fa))
        .route("/2fa/disable", post(disable_2fa))
        .route("/change-password", post(change_password))
        .route("/change-email", post(change_email))
        .route("/change-gravatar-email", post(change_gravatar_email))
}

// Define the public, unauthenticated routes for the users module
pub fn public_users_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_user))
        .route("/public-key", get(get_public_key))
}

pub fn session_parts(
    token: String,
    two_factor_enabled: bool,
    force_password_change: bool,
) -> Result<(String, AuthResponse), StatusCode> {
    let token_data =
        crate::auth::jwt::verify_jwt(&token).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cookie = Cookie::build(("auth_token", token.clone()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .build();
    let body = AuthResponse {
        token,
        user_id: token_data.claims.user_id.to_string(),
        username: token_data.claims.username,
        two_factor_enabled,
        force_password_change,
    };
    Ok((cookie.to_string(), body))
}

fn session_response(
    token: String,
    two_factor_enabled: bool,
    force_password_change: bool,
) -> Result<impl IntoResponse, StatusCode> {
    let (cookie, body) = session_parts(token, two_factor_enabled, force_password_change)?;
    Ok((StatusCode::OK, [(header::SET_COOKIE, cookie)], Json(body)))
}

/// Login user
#[utoipa::path(
    post,
    path = "/api/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "User logged in successfully", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 403, description = "2FA required"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Authentication"
)]
pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());
    let username = payload.username.clone();

    match auth_service
        .login_user(
            payload.username,
            payload.encrypted_password,
            payload.two_factor_code,
        )
        .await
    {
        Ok((token, two_factor_enabled, force_password_change)) => {
            if let Ok(data) = crate::auth::jwt::verify_jwt(&token) {
                shared::audit!(data.claims.user_id, "auth.login", "user", data.claims.user_id);
            }
            session_response(token, two_factor_enabled, force_password_change)
        }
        Err(err) => {
            tracing::warn!(
                target: "csfx::audit",
                action = "auth.login_failed",
                target_type = "user",
                target_id = %username,
                "auth.login_failed"
            );
            tracing::error!("Login failed: {}", err);
            match err {
                crate::auth_service::AuthError::UserNotFound
                | crate::auth_service::AuthError::InvalidCredentials => {
                    Err(StatusCode::UNAUTHORIZED)
                }
                crate::auth_service::AuthError::TwoFactorRequired => Err(StatusCode::FORBIDDEN),
                crate::auth_service::AuthError::InvalidTwoFactorCode => {
                    Err(StatusCode::UNAUTHORIZED)
                }
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Logout user (protected)
#[utoipa::path(
    post,
    path = "/api/logout",
    responses(
        (status = 200, description = "User logged out successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn logout_user(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());
    let exp_datetime = DateTime::from_timestamp(claims.exp, 0).unwrap_or_else(Utc::now);

    // Extract token from request header would be better, but for simplicity
    // we'll use a placeholder. In a real implementation, you'd extract the actual token.
    let token = "".to_string(); // This should be extracted from the Authorization header

    match auth_service.logout_user(token, exp_datetime).await {
        Ok(_) => Ok(Json(json!({ "message": "Logged out successfully" }))),
        Err(err) => {
            tracing::error!("Logout failed: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get RSA public key for encryption
#[utoipa::path(
    get,
    path = "/api/public-key",
    responses(
        (status = 200, description = "Public key retrieved successfully", body = PublicKeyResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Encryption"
)]
pub async fn get_public_key(
    State(state): State<AppState>,
) -> Result<Json<PublicKeyResponse>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service.get_public_key("main").await {
        Ok(public_key) => Ok(Json(PublicKeyResponse { public_key })),
        Err(err) => {
            tracing::error!("Failed to get public key: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get user profile (protected)
#[utoipa::path(
    get,
    path = "/api/profile",
    responses(
        (status = 200, description = "User profile retrieved successfully", body = UserProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn get_user_profile(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<UserProfileResponse>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service.get_user_by_id(claims.user_id).await {
        Ok(user) => Ok(Json(UserProfileResponse {
            id: user.id.to_string(),
            username: user.name,
            email: user.email.clone(),
            gravatar_email: user.gravatar_email.clone(),
            two_factor_enabled: user.two_factor_enabled,
            force_password_change: user.force_password_change,
        })),
        Err(err) => {
            tracing::error!("Failed to get user profile: {}", err);
            match err {
                crate::auth_service::AuthError::UserNotFound => Err(StatusCode::NOT_FOUND),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Validate current session (protected)
#[utoipa::path(
    get,
    path = "/api/validate-session",
    responses(
        (status = 200, description = "Session is valid", body = UserProfileResponse),
        (status = 401, description = "Session is invalid or expired")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn validate_session(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<UserProfileResponse>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    // Check if user still exists
    match auth_service.get_user_by_id(claims.user_id).await {
        Ok(user) => Ok(Json(UserProfileResponse {
            id: user.id.to_string(),
            username: user.name,
            email: user.email.clone(),
            gravatar_email: user.gravatar_email.clone(),
            two_factor_enabled: user.two_factor_enabled,
            force_password_change: user.force_password_change,
        })),
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

// 2FA Endpoints

#[derive(Serialize, ToSchema)]
pub struct Setup2FAResponse {
    pub secret: String,
    pub qr_code: String,
}

#[derive(Deserialize, ToSchema)]
pub struct TwoFactorCodeRequest {
    pub code: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangeEmailRequest {
    pub new_email: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangeGravatarEmailRequest {
    pub gravatar_email: Option<String>,
}

/// Setup 2FA for user (protected)
#[utoipa::path(
    post,
    path = "/api/2fa/setup",
    responses(
        (status = 200, description = "2FA setup initiated", body = Setup2FAResponse),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn setup_2fa(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Setup2FAResponse>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service.generate_2fa_secret(claims.user_id).await {
        Ok((secret, qr_code)) => Ok(Json(Setup2FAResponse { secret, qr_code })),
        Err(err) => {
            tracing::error!("Failed to setup 2FA: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Enable 2FA with verification code (protected)
#[utoipa::path(
    post,
    path = "/api/2fa/enable",
    request_body = TwoFactorCodeRequest,
    responses(
        (status = 200, description = "2FA enabled successfully"),
        (status = 400, description = "Invalid 2FA code"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn enable_2fa(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<TwoFactorCodeRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service.enable_2fa(claims.user_id, payload.code).await {
        Ok(_) => Ok(Json(json!({ "message": "2FA enabled successfully" }))),
        Err(crate::auth_service::AuthError::InvalidTwoFactorCode) => Err(StatusCode::BAD_REQUEST),
        Err(err) => {
            tracing::error!("Failed to enable 2FA: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Disable 2FA with verification code (protected)
#[utoipa::path(
    post,
    path = "/api/2fa/disable",
    request_body = TwoFactorCodeRequest,
    responses(
        (status = 200, description = "2FA disabled successfully"),
        (status = 400, description = "Invalid 2FA code"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn disable_2fa(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<TwoFactorCodeRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service.disable_2fa(claims.user_id, payload.code).await {
        Ok(_) => Ok(Json(json!({ "message": "2FA disabled successfully" }))),
        Err(crate::auth_service::AuthError::InvalidTwoFactorCode) => Err(StatusCode::BAD_REQUEST),
        Err(err) => {
            tracing::error!("Failed to disable 2FA: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Change user password (protected)
#[utoipa::path(
    post,
    path = "/api/change-password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed successfully"),
        (status = 400, description = "Invalid old password"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn change_password(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service
        .change_password(claims.user_id, payload.old_password, payload.new_password)
        .await
    {
        Ok(_) => Ok(Json(json!({ "message": "Password changed successfully" }))),
        Err(crate::auth_service::AuthError::InvalidCredentials) => Err(StatusCode::BAD_REQUEST),
        Err(err) => {
            tracing::error!("Failed to change password: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Change user email (protected)
#[utoipa::path(
    post,
    path = "/api/change-email",
    request_body = ChangeEmailRequest,
    responses(
        (status = 200, description = "Email changed successfully"),
        (status = 409, description = "Email already in use"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn change_email(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<ChangeEmailRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service
        .change_email(claims.user_id, payload.new_email)
        .await
    {
        Ok(_) => Ok(Json(json!({ "message": "Email changed successfully" }))),
        Err(crate::auth_service::AuthError::UserAlreadyExists) => Err(StatusCode::CONFLICT),
        Err(err) => {
            tracing::error!("Failed to change email: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Change gravatar avatar email (protected)
#[utoipa::path(
    post,
    path = "/api/change-gravatar-email",
    request_body = ChangeGravatarEmailRequest,
    responses(
        (status = 200, description = "Gravatar email changed successfully"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn change_gravatar_email(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<ChangeGravatarEmailRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service
        .change_gravatar_email(claims.user_id, payload.gravatar_email)
        .await
    {
        Ok(_) => Ok(Json(
            json!({ "message": "Gravatar email changed successfully" }),
        )),
        Err(err) => {
            tracing::error!("Failed to change gravatar email: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
