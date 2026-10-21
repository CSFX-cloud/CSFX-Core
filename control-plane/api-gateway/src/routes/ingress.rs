use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get},
    Router,
};
use chrono::Utc;
use entity::entities::{ingress_gateways, ingress_routes, resource_groups, workloads};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::cluster::{error, internal, ApiError};
use crate::{
    auth::rbac::{CanManageResourceGroups, CanViewResourceGroups},
    AppState,
};

const MAX_HOSTNAME_LENGTH: usize = 253;
const MAX_LABEL_LENGTH: usize = 63;

#[derive(Debug, Deserialize)]
pub struct CreateIngressRequest {
    pub hostname: String,
    pub gateway_id: Uuid,
    pub workload_id: Uuid,
    pub container_port: u16,
    #[serde(default = "default_tls")]
    pub tls: bool,
}

fn default_tls() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct IngressResponse {
    pub id: Uuid,
    pub resource_group_id: Uuid,
    pub workload_id: Uuid,
    pub gateway_id: Option<Uuid>,
    pub hostname: String,
    pub container_port: i32,
    pub tls: bool,
    pub created_at: String,
}

impl From<ingress_routes::Model> for IngressResponse {
    fn from(route: ingress_routes::Model) -> Self {
        Self {
            id: route.id,
            resource_group_id: route.resource_group_id,
            workload_id: route.workload_id,
            gateway_id: route.gateway_id,
            hostname: route.hostname,
            container_port: route.container_port,
            tls: route.tls,
            created_at: route.created_at.and_utc().to_rfc3339(),
        }
    }
}

pub fn validate_hostname(raw: &str) -> Result<String, &'static str> {
    let hostname = raw.trim().to_ascii_lowercase();
    if hostname.is_empty() || hostname.len() > MAX_HOSTNAME_LENGTH {
        return Err("hostname length is invalid");
    }
    let labels: Vec<&str> = hostname.split('.').collect();
    if labels.len() < 2 {
        return Err("hostname needs at least two labels");
    }
    if !labels.iter().all(|label| is_valid_label(label)) {
        return Err("hostname contains an invalid label");
    }
    if labels
        .last()
        .is_some_and(|label| label.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err("hostname must not be an ip address");
    }
    Ok(hostname)
}

pub(super) fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= MAX_LABEL_LENGTH
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub fn port_is_declared(ports: Option<&Value>, container_port: u16) -> bool {
    ports.and_then(Value::as_array).is_some_and(|mappings| {
        mappings
            .iter()
            .any(|mapping| mapping["container_port"].as_u64() == Some(u64::from(container_port)))
    })
}

pub(super) async fn ensure_group(
    state: &AppState,
    group_id: Uuid,
) -> Result<resource_groups::Model, ApiError> {
    let org_id = state
        .default_org_id
        .ok_or_else(|| internal("default organization not initialized"))?;
    resource_groups::Entity::find_by_id(group_id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "resource group not found"))
}

pub async fn list_ingress(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<Vec<IngressResponse>>, ApiError> {
    ensure_group(&state, group_id).await?;
    let routes = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::ResourceGroupId.eq(group_id))
        .order_by_asc(ingress_routes::Column::Hostname)
        .all(&state.db_conn)
        .await
        .map_err(internal)?;
    Ok(Json(routes.into_iter().map(Into::into).collect()))
}

pub async fn create_ingress(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<CreateIngressRequest>,
) -> Result<(StatusCode, Json<IngressResponse>), ApiError> {
    ensure_group(&state, group_id).await?;
    let hostname = validate_hostname(&request.hostname)
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    ensure_gateway(&state, group_id, request.gateway_id).await?;
    ensure_workload_port(&state, group_id, &request).await?;
    ensure_hostname_free(&state, &hostname).await?;

    let route = ingress_routes::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group_id),
        workload_id: Set(request.workload_id),
        gateway_id: Set(Some(request.gateway_id)),
        hostname: Set(hostname),
        container_port: Set(i32::from(request.container_port)),
        tls: Set(request.tls),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await
    .map_err(internal)?;

    tracing::info!(
        user_id = %claims.user_id,
        route_id = %route.id,
        hostname = %route.hostname,
        "ingress route created"
    );
    Ok((StatusCode::CREATED, Json(route.into())))
}

async fn ensure_gateway(
    state: &AppState,
    group_id: Uuid,
    gateway_id: Uuid,
) -> Result<(), ApiError> {
    ingress_gateways::Entity::find_by_id(gateway_id)
        .filter(ingress_gateways::Column::ResourceGroupId.eq(group_id))
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .map(|_| ())
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                "create an ingress gateway in the resource group first",
            )
        })
}

async fn ensure_workload_port(
    state: &AppState,
    group_id: Uuid,
    request: &CreateIngressRequest,
) -> Result<(), ApiError> {
    let workload = workloads::Entity::find_by_id(request.workload_id)
        .filter(workloads::Column::ResourceGroupId.eq(group_id))
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "workload not found in resource group",
            )
        })?;
    if !port_is_declared(workload.ports.as_ref(), request.container_port) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "container port is not declared by the workload",
        ));
    }
    Ok(())
}

async fn ensure_hostname_free(state: &AppState, hostname: &str) -> Result<(), ApiError> {
    let taken = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::Hostname.eq(hostname))
        .count(&state.db_conn)
        .await
        .map_err(internal)?;
    if taken > 0 {
        return Err(error(StatusCode::CONFLICT, "hostname is already routed"));
    }
    Ok(())
}

pub async fn delete_ingress(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, route_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    ensure_group(&state, group_id).await?;
    let result = ingress_routes::Entity::delete_many()
        .filter(ingress_routes::Column::Id.eq(route_id))
        .filter(ingress_routes::Column::ResourceGroupId.eq(group_id))
        .exec(&state.db_conn)
        .await
        .map_err(internal)?;
    if result.rows_affected == 0 {
        return Err(error(StatusCode::NOT_FOUND, "ingress route not found"));
    }

    tracing::info!(user_id = %claims.user_id, route_id = %route_id, "ingress route deleted");
    Ok(StatusCode::NO_CONTENT)
}

pub fn ingress_routes_router() -> Router<AppState> {
    Router::new()
        .route(
            "/resource-groups/{id}/ingress",
            get(list_ingress).post(create_ingress),
        )
        .route(
            "/resource-groups/{id}/ingress/{route_id}",
            delete(delete_ingress),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hostname_is_normalized_and_accepted() {
        assert_eq!(
            validate_hostname(" App.Example.COM "),
            Ok("app.example.com".to_string())
        );
        assert_eq!(validate_hostname("a-b.c1.io"), Ok("a-b.c1.io".to_string()));
    }

    #[test]
    fn invalid_hostnames_are_rejected() {
        for hostname in [
            "",
            "localhost",
            "-bad.example.com",
            "bad-.example.com",
            "bad_name.example.com",
            "*.example.com",
            "a..example.com",
            "192.168.1.10",
            "has space.example.com",
        ] {
            assert!(validate_hostname(hostname).is_err(), "{hostname}");
        }
    }

    #[test]
    fn overlong_label_and_hostname_are_rejected() {
        let label = "a".repeat(64);
        assert!(validate_hostname(&format!("{label}.example.com")).is_err());
        let long = format!("{}.com", "a.".repeat(130));
        assert!(validate_hostname(&long).is_err());
    }

    #[test]
    fn declared_port_is_found_in_workload_ports() {
        let ports = json!([{"container_port": 80}, {"container_port": 8080, "node_port": 30002}]);
        assert!(port_is_declared(Some(&ports), 8080));
        assert!(!port_is_declared(Some(&ports), 81));
        assert!(!port_is_declared(Some(&json!(null)), 80));
        assert!(!port_is_declared(None, 80));
    }
}
