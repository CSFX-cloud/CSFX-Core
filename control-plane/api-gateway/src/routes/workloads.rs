use axum::{
    body::Body,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{delete, get, patch, post},
    Router,
};
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use site_builder::images::{
    configured_registry_endpoints, mentions_site_repository, uses_site_registry, SITE_IMAGE_HOST,
};
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageWorkloads, CanViewWorkloads},
    routes::paged_proxy::{proxy_list, with_query},
    service_client::ListTarget,
    AppState,
};

const RESERVED_IMAGE_HOST_MESSAGE: &str = "this image host is reserved for web apps";

fn references_site_images(body: &Value) -> bool {
    references_site_images_with(body, &configured_registry_endpoints())
}

fn references_site_images_with(body: &Value, endpoints: &[String]) -> bool {
    let image = body["image"].as_str().is_some_and(|image| {
        uses_site_registry(image) || mentions_site_repository(image, endpoints)
    });
    let compose = body["compose_yaml"].as_str().is_some_and(|yaml| {
        yaml.to_ascii_lowercase().contains(SITE_IMAGE_HOST)
            || mentions_site_repository(yaml, endpoints)
    });
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

const FOREIGN_GROUP_MESSAGE: &str = "images from another resource group's registry cannot be used";
const GROUP_MARKER: &str = "rg-";
const GROUP_ID_LENGTH: usize = 32;

fn group_references(text: &str) -> Vec<String> {
    let lowered = text.to_ascii_lowercase();
    lowered
        .match_indices(GROUP_MARKER)
        .filter_map(|(start, _)| {
            let id_start = start + GROUP_MARKER.len();
            let id = lowered.get(id_start..id_start + GROUP_ID_LENGTH)?;
            let is_id = id.chars().all(|c| c.is_ascii_hexdigit());
            let slash = lowered.get(id_start + GROUP_ID_LENGTH..)?.starts_with('/');
            (is_id && slash).then(|| id.to_string())
        })
        .collect()
}

fn references_foreign_group(body: &Value, own_group: Option<Uuid>) -> bool {
    let own = own_group.map(|id| id.simple().to_string());
    let texts = [body["image"].as_str(), body["compose_yaml"].as_str()];
    texts
        .into_iter()
        .flatten()
        .flat_map(group_references)
        .any(|id| own.as_deref() != Some(id.as_str()))
}

fn reject_foreign_group_images(
    body: &Option<Value>,
    own_group: Option<Uuid>,
) -> Result<(), (StatusCode, Json<Value>)> {
    if body
        .as_ref()
        .is_some_and(|body| references_foreign_group(body, own_group))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": FOREIGN_GROUP_MESSAGE })),
        ));
    }
    Ok(())
}

async fn workload_group(state: &AppState, id: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(id).ok()?;
    entity::workloads::Entity::find_by_id(id)
        .one(&state.db_conn)
        .await
        .ok()
        .flatten()
        .and_then(|workload| workload.resource_group_id)
}

async fn stack_group(state: &AppState, id: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(id).ok()?;
    entity::workload_stacks::Entity::find_by_id(id)
        .one(&state.db_conn)
        .await
        .ok()
        .flatten()
        .map(|stack| stack.resource_group_id)
}

fn body_group(body: &Option<Value>) -> Option<Uuid> {
    body.as_ref()?["resource_group_id"]
        .as_str()
        .and_then(|raw| Uuid::parse_str(raw).ok())
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
    reject_foreign_group_images(&body_json, body_group(&body_json))?;
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
    RawQuery(query): RawQuery,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let path = with_query("/workloads", query);
    proxy_list(&state, ListTarget::Scheduler, &path, &headers).await
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
    reject_foreign_group_images(&body_json, workload_group(&state, &id).await)?;
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
    reject_foreign_group_images(&body_json, body_group(&body_json))?;
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
    reject_foreign_group_images(&body_json, stack_group(&state, &id).await)?;
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
    fn site_repositories_on_the_registry_endpoint_are_rejected() {
        let endpoints = vec!["10.0.0.5:5060".to_string()];
        let site = json!({ "image": "10.0.0.5:5060/00000000-0000-0000-0000-000000000000:abc" });
        assert!(references_site_images_with(&site, &endpoints));
        let own = json!({ "image": "10.0.0.5:5060/rg-abc/main/app:1" });
        assert!(!references_site_images_with(&own, &endpoints));
    }

    #[test]
    fn only_the_own_groups_registry_namespace_is_allowed() {
        let own = Uuid::from_u128(0xabc);
        let other = Uuid::from_u128(0xdef);
        let image = |group: Uuid| json!({ "image": format!("10.0.0.5:5060/rg-{}/main/app:1", group.simple()) });
        assert!(!references_foreign_group(&image(own), Some(own)));
        assert!(references_foreign_group(&image(other), Some(own)));
        assert!(references_foreign_group(&image(own), None));
        assert!(!references_foreign_group(
            &json!({ "image": "nginx:1" }),
            None
        ));
        let compose = format!(
            "services:\n  a:\n    image: host/rg-{}/x/app\n",
            other.simple()
        );
        assert!(references_foreign_group(
            &json!({ "compose_yaml": compose }),
            Some(own)
        ));
    }

    #[test]
    fn lookalike_names_are_not_group_references() {
        assert!(group_references("registry/rg-short/x").is_empty());
        assert!(group_references("rg-zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz/x").is_empty());
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
