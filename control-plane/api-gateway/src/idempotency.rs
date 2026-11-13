use axum::{
    body::{to_bytes, Body, Bytes},
    extract::{Request, State},
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use entity::entities::idempotency_keys;
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, Condition, DbConn, DbErr, EntityTrait, QueryFilter,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::time::{interval, Duration};

use crate::auth::{jwt::verify_jwt, middleware::request_token};
use crate::AppState;

const KEY_HEADER: &str = "idempotency-key";
const REPLAY_HEADER: &str = "idempotent-replayed";
const MAX_KEY_LENGTH: usize = 255;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const RETENTION_HOURS: i64 = 24;
const RESERVATION_MINUTES: i64 = 5;
const PRUNE_INTERVAL: Duration = Duration::from_secs(600);
const PENDING_STATUS: i32 = 0;

#[derive(Debug, PartialEq)]
pub enum Decision {
    Replay,
    Mismatch,
    InFlight,
}

pub fn decide(existing: &idempotency_keys::Model, request_hash: &str) -> Decision {
    if existing.request_hash != request_hash {
        return Decision::Mismatch;
    }
    if existing.response_status == PENDING_STATUS {
        return Decision::InFlight;
    }
    Decision::Replay
}

pub fn request_hash(method: &Method, target: &str, body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(method.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(target.as_bytes());
    hasher.update([0]);
    hasher.update(body);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn rejection(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn replay(stored: idempotency_keys::Model) -> Response {
    let status = u16::try_from(stored.response_status)
        .ok()
        .and_then(|code| StatusCode::from_u16(code).ok())
        .unwrap_or(StatusCode::OK);
    let mut response = (status, stored.response_body).into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
        .headers_mut()
        .insert(REPLAY_HEADER, HeaderValue::from_static("true"));
    response
}

fn scope_of(request: &Request) -> Option<String> {
    let token = request_token(request.headers())?;
    let claims = verify_jwt(token).ok()?.claims;
    Some(format!("user:{}", claims.user_id))
}

fn key_of(request: &Request) -> Option<Result<String, Response>> {
    let value = request.headers().get(KEY_HEADER)?;
    let key = value.to_str().unwrap_or_default().trim();
    if key.is_empty() || key.len() > MAX_KEY_LENGTH {
        return Some(Err(rejection(
            StatusCode::BAD_REQUEST,
            "idempotency key must be 1 to 255 characters",
        )));
    }
    Some(Ok(key.to_string()))
}

async fn reserve(
    db: &DbConn,
    scope: &str,
    key: &str,
    target: &str,
    hash: &str,
) -> Result<bool, DbErr> {
    let row = idempotency_keys::ActiveModel {
        scope: sea_orm::Set(scope.to_string()),
        key: sea_orm::Set(key.to_string()),
        path: sea_orm::Set(target.to_string()),
        request_hash: sea_orm::Set(hash.to_string()),
        response_status: sea_orm::Set(PENDING_STATUS),
        response_body: sea_orm::Set(Vec::new()),
        created_at: sea_orm::Set(Utc::now().naive_utc()),
    };
    let inserted = idempotency_keys::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                idempotency_keys::Column::Scope,
                idempotency_keys::Column::Key,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(inserted == 1)
}

async fn complete(
    db: &DbConn,
    scope: &str,
    key: &str,
    status: StatusCode,
    body: &Bytes,
) -> Result<(), DbErr> {
    idempotency_keys::Entity::update_many()
        .col_expr(
            idempotency_keys::Column::ResponseStatus,
            Expr::value(i32::from(status.as_u16())),
        )
        .col_expr(
            idempotency_keys::Column::ResponseBody,
            Expr::value(body.to_vec()),
        )
        .filter(idempotency_keys::Column::Scope.eq(scope))
        .filter(idempotency_keys::Column::Key.eq(key))
        .exec(db)
        .await?;
    Ok(())
}

async fn release(db: &DbConn, scope: &str, key: &str) {
    if let Err(error) = idempotency_keys::Entity::delete_by_id((scope.to_string(), key.to_string()))
        .exec(db)
        .await
    {
        tracing::warn!(error = %error, "idempotency reservation could not be released");
    }
}

async fn existing_response(db: &DbConn, scope: &str, key: &str, hash: &str) -> Response {
    let found = idempotency_keys::Entity::find_by_id((scope.to_string(), key.to_string()))
        .one(db)
        .await;
    match found {
        Ok(Some(stored)) => match decide(&stored, hash) {
            Decision::Replay => replay(stored),
            Decision::Mismatch => rejection(
                StatusCode::UNPROCESSABLE_ENTITY,
                "idempotency key was used with a different request",
            ),
            Decision::InFlight => rejection(
                StatusCode::CONFLICT,
                "a request with this idempotency key is still in progress",
            ),
        },
        Ok(None) => rejection(StatusCode::CONFLICT, "idempotency key was just released"),
        Err(error) => {
            tracing::error!(error = %error, "idempotency lookup failed");
            rejection(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

async fn execute(
    state: &AppState,
    scope: &str,
    key: &str,
    request: Request,
    next: Next,
) -> Response {
    let response = next.run(request).await;
    let (parts, body) = response.into_parts();
    let Ok(bytes) = to_bytes(body, MAX_BODY_BYTES).await else {
        release(&state.db_conn, scope, key).await;
        return rejection(
            StatusCode::INTERNAL_SERVER_ERROR,
            "response body unavailable",
        );
    };
    if !parts.status.is_success() {
        release(&state.db_conn, scope, key).await;
        return Response::from_parts(parts, Body::from(bytes));
    }
    if let Err(error) = complete(&state.db_conn, scope, key, parts.status, &bytes).await {
        tracing::error!(error = %error, "idempotency result could not be stored");
    }
    Response::from_parts(parts, Body::from(bytes))
}

pub async fn guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if request.method() != Method::POST {
        return next.run(request).await;
    }
    let key = match key_of(&request) {
        None => return next.run(request).await,
        Some(Err(response)) => return response,
        Some(Ok(key)) => key,
    };
    let Some(scope) = scope_of(&request) else {
        return next.run(request).await;
    };
    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, MAX_BODY_BYTES).await else {
        return rejection(StatusCode::PAYLOAD_TOO_LARGE, "request body too large");
    };
    let target = parts
        .uri
        .path_and_query()
        .map_or_else(|| parts.uri.path().to_string(), ToString::to_string);
    let hash = request_hash(&parts.method, &target, &bytes);
    match reserve(&state.db_conn, &scope, &key, &target, &hash).await {
        Ok(true) => {
            let request = Request::from_parts(parts, Body::from(bytes));
            execute(&state, &scope, &key, request, next).await
        }
        Ok(false) => existing_response(&state.db_conn, &scope, &key, &hash).await,
        Err(error) => {
            tracing::error!(error = %error, "idempotency reservation failed");
            rejection(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

async fn prune_once(db: &DbConn) -> Result<u64, DbErr> {
    let now = Utc::now();
    let expired = Condition::any()
        .add(
            idempotency_keys::Column::CreatedAt
                .lt((now - chrono::Duration::hours(RETENTION_HOURS)).naive_utc()),
        )
        .add(
            Condition::all()
                .add(idempotency_keys::Column::ResponseStatus.eq(PENDING_STATUS))
                .add(
                    idempotency_keys::Column::CreatedAt
                        .lt((now - chrono::Duration::minutes(RESERVATION_MINUTES)).naive_utc()),
                ),
        );
    let result = idempotency_keys::Entity::delete_many()
        .filter(expired)
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

pub fn spawn_prune_job(db: DbConn) {
    tokio::spawn(async move {
        let mut ticker = interval(PRUNE_INTERVAL);
        loop {
            ticker.tick().await;
            match prune_once(&db).await {
                Ok(rows_deleted) if rows_deleted > 0 => {
                    tracing::info!(rows_deleted = rows_deleted, "pruned idempotency keys");
                }
                Ok(_) => {}
                Err(error) => tracing::error!(error = %error, "failed to prune idempotency keys"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(hash: &str, status: i32) -> idempotency_keys::Model {
        idempotency_keys::Model {
            scope: "user:1".into(),
            key: "k".into(),
            path: "/workloads".into(),
            request_hash: hash.into(),
            response_status: status,
            response_body: Vec::new(),
            created_at: Utc::now().naive_utc(),
        }
    }

    #[test]
    fn same_request_replays_the_stored_response() {
        assert_eq!(decide(&stored("a", 201), "a"), Decision::Replay);
    }

    #[test]
    fn different_request_under_the_same_key_is_a_mismatch() {
        assert_eq!(decide(&stored("a", 201), "b"), Decision::Mismatch);
        assert_eq!(decide(&stored("a", 0), "b"), Decision::Mismatch);
    }

    #[test]
    fn unfinished_request_is_in_flight() {
        assert_eq!(decide(&stored("a", 0), "a"), Decision::InFlight);
    }

    #[test]
    fn hash_covers_method_target_and_body() {
        let base = request_hash(&Method::POST, "/workloads", b"{}");
        assert_eq!(base, request_hash(&Method::POST, "/workloads", b"{}"));
        assert_ne!(base, request_hash(&Method::POST, "/volumes", b"{}"));
        assert_ne!(base, request_hash(&Method::POST, "/workloads", b"{ }"));
        assert_ne!(base, request_hash(&Method::PUT, "/workloads", b"{}"));
    }
}
