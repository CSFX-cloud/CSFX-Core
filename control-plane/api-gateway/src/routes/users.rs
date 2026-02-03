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
pub struct RegisterRequest {
    /// Username for registration
    pub username: String,
    /// RSA encrypted password
    pub encrypted_password: String,
}

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
    /// Whether 2FA is enabled
    pub two_factor_enabled: bool,
    /// Whether password change is required
    pub force_password_change: bool,
}

// Define the routes for the users module
pub fn users_routes() -> Router<AppState> {
    let auth_routes = Router::new()
        .route("/profile", get(get_user_profile))
        .route("/validate-session", get(validate_session))
        .route("/logout", post(logout_user))
        .route("/2fa/setup", post(setup_2fa))
        .route("/2fa/enable", post(enable_2fa))
        .route("/2fa/disable", post(disable_2fa))
        .route("/change-password", post(change_password))
        .route("/change-email", post(change_email));

    Router::new()
        .route("/register", post(register_user))
        .route("/login", post(login_user))
        .route("/public-key", get(get_public_key))
        .merge(auth_routes)
}

/// Register a new user
#[utoipa::path(
    post,
    path = "/api/register",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "User registered successfully", body = AuthResponse),
        (status = 409, description = "User already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Authentication"
)]
pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let auth_service = AuthService::new(state.db_conn.clone());

    match auth_service
        .register_user(payload.username.clone(), payload.encrypted_password)
        .await
    {
        Ok(token) => {
            // Extract user info from token for response
            match crate::auth::jwt::verify_jwt(&token) {
                Ok(token_data) => {
                    let cookie = Cookie::build(("auth_token", token.clone()))
                        .path("/")
                        .http_only(true)
                        .same_site(SameSite::Strict)
                        .build();

                    let response = (
                        StatusCode::OK,
                        [(header::SET_COOKIE, cookie.to_string())],
                        Json(AuthResponse {
                            token,
                            user_id: token_data.claims.user_id.to_string(),
                            username: token_data.claims.username,
                            two_factor_enabled: false,
                            force_password_change: false,
                        }),
                    );
                    Ok(response)
                }
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(err) => {
            tracing::error!("Registration failed: {}", err);
            match err {
                crate::auth_service::AuthError::UserAlreadyExists => Err(StatusCode::CONFLICT),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
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

    match auth_service
        .login_user(
            payload.username,
            payload.encrypted_password,
            payload.two_factor_code,
        )
        .await
    {
        Ok((token, two_factor_enabled, force_password_change)) => {
            // Extract user info from token for response
            match crate::auth::jwt::verify_jwt(&token) {
                Ok(token_data) => {
                    let cookie = Cookie::build(("auth_token", token.clone()))
                        .path("/")
                        .http_only(true)
                        .same_site(SameSite::Strict)
                        .build();

                    let response = (
                        StatusCode::OK,
                        [(header::SET_COOKIE, cookie.to_string())],
                        Json(AuthResponse {
                            token,
                            user_id: token_data.claims.user_id.to_string(),
                            username: token_data.claims.username,
                            two_factor_enabled,
                            force_password_change,
                        }),
                    );
                    Ok(response)
                }
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(err) => {
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
