use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{delete, get, patch, post},
    Router,
};
use serde_json::{json, Value};
use site_builder::images::{uses_site_registry, SITE_IMAGE_HOST};

use crate::{
    auth::rbac::{CanManageWorkloads, CanViewWorkloads},
    AppState,
};

const RESERVED_IMAGE_HOST_MESSAGE: &str = "this image host is reserved for web apps";

fn references_site_images(body: &Value) -> bool {
    let image = body["image"].as_str().is_some_and(uses_site_registry);
    let compose = body["compose_yaml"]
        .as_str()
        .is_some_and(|yaml| yaml.to_ascii_lowercase().contains(SITE_IMAGE_HOST));
    image || compose
}

fn reject_site_images(body: &Option<Value>) -> Result<(), (StatusCode, Json<Value>)> {
    if body.as_ref().is_some_and(references_site_images) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": RESERVED_IMAGE_HOST_MESSAGE })),
        ));
    }
    Ok(())
}

async fn proxy_to_scheduler(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    headers: Option<Vec<(String, String)>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state
        .service_client
        .forward_to_scheduler(method, path, body, headers)
        .await
    {
        Ok((status, Some(body))) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Json(body)).into_response())
        }
        Ok((status, None)) => {
            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok((axum_status, Body::empty()).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to forward request to scheduler: {}", e);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "Scheduler service unavailable", "details": e.to_string() })),
            ))
        }
    }
}

pub async fn create_workload(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    reject_site_images(&body_json)?;
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::POST,
        "/workloads",
        body_json,
        Some(header_map),
    )
    .await
}

pub async fn list_workloads(
    CanViewWorkloads(_claims): CanViewWorkloads,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::GET,
        "/workloads",
        None,
        Some(header_map),
    )
    .await
}

pub async fn delete_workload(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::DELETE,
        &format!("/workloads/{}", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn update_workload(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    reject_site_images(&body_json)?;
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::PATCH,
        &format!("/workloads/{}", id),
        body_json,
        Some(header_map),
    )
    .await
}

pub async fn stop_workload(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::POST,
        &format!("/workloads/{}/stop", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn restart_workload(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::POST,
        &format!("/workloads/{}/restart", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn create_stack(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    reject_site_images(&body_json)?;
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::POST,
        "/workload-stacks",
        body_json,
        Some(header_map),
    )
    .await
}

pub async fn get_stack(
    CanViewWorkloads(_claims): CanViewWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::GET,
        &format!("/workload-stacks/{}", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn delete_stack(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::DELETE,
        &format!("/workload-stacks/{}", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn stop_stack(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::POST,
        &format!("/workload-stacks/{}/stop", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn restart_stack(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::POST,
        &format!("/workload-stacks/{}/restart", id),
        None,
        Some(header_map),
    )
    .await
}

pub async fn redeploy_stack(
    CanManageWorkloads(_claims): CanManageWorkloads,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    reject_site_images(&body_json)?;
    let header_map = header_vec(&headers);
    proxy_to_scheduler(
        &state,
        reqwest::Method::PATCH,
        &format!("/workload-stacks/{}", id),
        body_json,
        Some(header_map),
    )
    .await
}

fn header_vec(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect()
}

pub fn workloads_routes() -> Router<AppState> {
    Router::new()
        .route("/workloads", post(create_workload))
        .route("/workloads", get(list_workloads))
        .route("/workloads/{id}", delete(delete_workload))
        .route("/workloads/{id}", patch(update_workload))
        .route("/workloads/{id}/stop", post(stop_workload))
        .route("/workloads/{id}/restart", post(restart_workload))
        .route("/workload-stacks", post(create_stack))
        .route("/workload-stacks/{id}", get(get_stack))
        .route("/workload-stacks/{id}", delete(delete_stack))
        .route("/workload-stacks/{id}/stop", post(stop_stack))
        .route("/workload-stacks/{id}/restart", post(restart_stack))
        .route("/workload-stacks/{id}", patch(redeploy_stack))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_image_hosts_are_rejected_in_images_and_compose_files() {
        assert!(references_site_images(
            &json!({ "image": "sites.csfx.invalid/x:1" })
        ));
        let compose = "services:\n  a:\n    image: Sites.CSFX.invalid/x:1\n";
        assert!(references_site_images(&json!({ "compose_yaml": compose })));
    }

    #[test]
    fn ordinary_workloads_pass() {
        assert!(!references_site_images(&json!({ "image": "nginx:1" })));
        assert!(!references_site_images(
            &json!({ "compose_yaml": "services: {}" })
        ));
        assert!(reject_site_images(&None).is_ok());
    }
}
