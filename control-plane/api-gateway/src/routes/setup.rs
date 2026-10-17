use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    auth::jwt::create_jwt,
    auth_service::AuthService,
    console_pin,
    routes::users::{session_parts, AuthResponse},
    setup::{
        ca_fingerprint, ca_pem, code_matches, complete_setup, is_completed, queue_join_request,
        read_setup_code, SetupError, SetupInput,
    },
    AppState,
};

type ApiError = (StatusCode, Json<Value>);

#[derive(Serialize)]
pub struct SetupStatusResponse {
    pub state: &'static str,
    pub version: &'static str,
    pub ca_fingerprint: Option<String>,
}

#[derive(Deserialize)]
pub struct SetupRequest {
    pub setup_code: String,
    pub organization_name: String,
    pub admin_email: String,
    pub encrypted_password: String,
    pub encrypted_console_pin: String,
}

#[derive(Deserialize)]
pub struct JoinRequest {
    pub setup_code: String,
    pub join_string: String,
}

#[derive(Serialize)]
pub struct SetupResponse {
    #[serde(flatten)]
    pub session: AuthResponse,
    pub recovery_key: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/setup/status", get(get_status))
        .route("/setup/ca.crt", get(get_ca_certificate))
        .route("/setup", post(submit_setup))
        .route("/setup/join", post(submit_join))
}

fn error_response(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

fn map_setup_error(err: SetupError) -> ApiError {
    match err {
        SetupError::AlreadyCompleted => error_response(StatusCode::GONE, "setup already completed"),
        SetupError::InvalidCode => error_response(StatusCode::UNAUTHORIZED, "invalid setup code"),
        SetupError::CodeUnavailable => {
            error_response(StatusCode::SERVICE_UNAVAILABLE, "setup code unavailable")
        }
        SetupError::InvalidInput(field) => error_response(
            StatusCode::BAD_REQUEST,
            &format!("invalid value for {field}"),
        ),
        other => {
            tracing::error!(error = %other, "setup failed");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "setup failed")
        }
    }
}

fn internal_error(err: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %err, "setup request failed");
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

pub async fn get_status(
    State(state): State<AppState>,
) -> Result<Json<SetupStatusResponse>, ApiError> {
    let completed = is_completed(&state.db_conn).await.map_err(internal_error)?;
    Ok(Json(SetupStatusResponse {
        state: if completed { "ready" } else { "setup_required" },
        version: env!("CARGO_PKG_VERSION"),
        ca_fingerprint: ca_fingerprint(),
    }))
}

pub async fn get_ca_certificate() -> Result<Response, StatusCode> {
    let pem = ca_pem().ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-pem-file"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"csfx-ca.crt\"",
            ),
        ],
        pem,
    )
        .into_response())
}

fn verify_setup_code(provided: &str) -> Result<(), SetupError> {
    let expected = read_setup_code().ok_or(SetupError::CodeUnavailable)?;
    if code_matches(&expected, provided) {
        return Ok(());
    }
    tracing::warn!(event = "setup.invalid_code");
    Err(SetupError::InvalidCode)
}

async fn decrypt_field(
    auth_service: &AuthService,
    encrypted: &str,
    field: &str,
) -> Result<String, ApiError> {
    auth_service
        .decrypt_client_password(encrypted)
        .await
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, &format!("invalid {field} payload")))
}

pub async fn submit_setup(
    State(state): State<AppState>,
    Json(request): Json<SetupRequest>,
) -> Result<Response, ApiError> {
    if is_completed(&state.db_conn).await.map_err(internal_error)? {
        return Err(map_setup_error(SetupError::AlreadyCompleted));
    }
    verify_setup_code(&request.setup_code).map_err(map_setup_error)?;
    let auth_service = AuthService::new(state.db_conn.clone());
    let password = decrypt_field(&auth_service, &request.encrypted_password, "password").await?;
    let console_pin =
        decrypt_field(&auth_service, &request.encrypted_console_pin, "console pin").await?;
    let admin_email = request.admin_email.trim().to_string();
    let input = SetupInput {
        organization_name: request.organization_name,
        admin_email: admin_email.clone(),
        password,
        console_pin: console_pin.clone(),
    };
    let admin_id = complete_setup(&state.db_conn, input)
        .await
        .map_err(map_setup_error)?;
    let recovery_key = provision_console_pin(&console_pin)?;
    tracing::info!(target: "csfx::audit", event = "setup.completed", admin = %admin_email);
    let token = create_jwt(admin_id, admin_email).map_err(internal_error)?;
    let (cookie, session) = session_parts(token, false, false)
        .map_err(|status| error_response(status, "session creation failed"))?;
    let body = Json(SetupResponse {
        session,
        recovery_key,
    });
    Ok((StatusCode::CREATED, [(header::SET_COOKIE, cookie)], body).into_response())
}

pub async fn submit_join(
    State(state): State<AppState>,
    Json(request): Json<JoinRequest>,
) -> Result<StatusCode, ApiError> {
    if is_completed(&state.db_conn).await.map_err(internal_error)? {
        return Err(map_setup_error(SetupError::AlreadyCompleted));
    }
    verify_setup_code(&request.setup_code).map_err(map_setup_error)?;
    queue_join_request(&request.join_string).map_err(map_setup_error)?;
    tracing::info!(target: "csfx::audit", event = "setup.join_requested");
    Ok(StatusCode::ACCEPTED)
}

fn provision_console_pin(pin: &str) -> Result<String, ApiError> {
    console_pin::provision(pin).map_err(|err| {
        tracing::error!(error = %err, "console pin provisioning failed");
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "console pin provisioning failed",
        )
    })
}
