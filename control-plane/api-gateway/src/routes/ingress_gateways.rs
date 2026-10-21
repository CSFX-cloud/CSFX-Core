use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get},
    Router,
};
use chrono::Utc;
use entity::entities::{ingress_gateways, workloads};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

use super::cluster::{error, internal, ApiError};
use super::ingress::{ensure_group, is_valid_label};
use crate::{
    auth::{
        agent::AgentApiKey,
        rbac::{CanManageResourceGroups, CanViewResourceGroups},
    },
    dns_sync, ingress_sync, zone_cert, AppState,
};

const GATEWAY_IMAGE: &str = "traefik:v3.5";
const GATEWAY_CPU_MILLICORES: i32 = 250;
const GATEWAY_MEMORY_BYTES: i64 = 256 * 1024 * 1024;
const GATEWAY_DISK_BYTES: i64 = 1024 * 1024 * 1024;
const DEFAULT_HTTP_PORT: u16 = 80;
const DEFAULT_HTTPS_PORT: u16 = 443;
const DEFAULT_AGENT_PORT: u16 = 7443;
const POLL_INTERVAL: &str = "10s";
const MAX_NAME_LENGTH: usize = 40;

#[derive(Debug, Deserialize)]
pub struct CreateGatewayRequest {
    pub name: String,
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default = "default_https_port")]
    pub https_port: u16,
}

fn default_http_port() -> u16 {
    DEFAULT_HTTP_PORT
}

fn default_https_port() -> u16 {
    DEFAULT_HTTPS_PORT
}

#[derive(Debug, Serialize)]
pub struct GatewayResponse {
    pub id: Uuid,
    pub resource_group_id: Uuid,
    pub workload_id: Uuid,
    pub name: String,
    pub http_port: i32,
    pub https_port: i32,
    pub endpoint: String,
    pub status: Option<String>,
    pub created_at: String,
}

fn endpoint_for(gateway_name: &str, group_name: &str, zone: &str) -> String {
    format!("{gateway_name}.{}.{zone}", group_name.to_ascii_lowercase())
}

fn response_for(
    gateway: ingress_gateways::Model,
    group_name: &str,
    status: Option<String>,
) -> GatewayResponse {
    GatewayResponse {
        endpoint: endpoint_for(&gateway.name, group_name, &dns_sync::zone_from_env()),
        id: gateway.id,
        resource_group_id: gateway.resource_group_id,
        workload_id: gateway.workload_id,
        name: gateway.name,
        http_port: gateway.http_port,
        https_port: gateway.https_port,
        status,
        created_at: gateway.created_at.and_utc().to_rfc3339(),
    }
}

pub fn validate_request(request: &CreateGatewayRequest) -> Result<String, &'static str> {
    let name = request.name.trim().to_ascii_lowercase();
    if name.len() > MAX_NAME_LENGTH || !is_valid_label(&name) {
        return Err("gateway name must be a dns label of at most 40 characters");
    }
    if request.http_port == 0 || request.https_port == 0 {
        return Err("gateway ports must not be zero");
    }
    if request.http_port == request.https_port {
        return Err("http and https port must differ");
    }
    Ok(name)
}

pub fn provider_endpoint(agent_port: u16, group_id: Uuid, gateway_id: Uuid) -> String {
    format!(
        "http://csfx.svc.{group_id}.internal:{agent_port}/ingress-config/{group_id}/{gateway_id}"
    )
}

pub fn gateway_environment(
    provider_endpoint: &str,
    acme: Option<(&str, &str)>,
) -> HashMap<String, String> {
    let mut env = HashMap::from([
        (
            "TRAEFIK_ENTRYPOINTS_WEB_ADDRESS".to_string(),
            ":80".to_string(),
        ),
        (
            "TRAEFIK_ENTRYPOINTS_WEBSECURE_ADDRESS".to_string(),
            ":443".to_string(),
        ),
        (
            "TRAEFIK_PROVIDERS_HTTP_ENDPOINT".to_string(),
            provider_endpoint.to_string(),
        ),
        (
            "TRAEFIK_PROVIDERS_HTTP_POLLINTERVAL".to_string(),
            POLL_INTERVAL.to_string(),
        ),
    ]);
    if let Some((resolver, email)) = acme {
        let prefix = format!(
            "TRAEFIK_CERTIFICATESRESOLVERS_{}_ACME",
            resolver.to_ascii_uppercase()
        );
        env.insert(format!("{prefix}_EMAIL"), email.to_string());
        env.insert(format!("{prefix}_STORAGE"), "/tmp/acme.json".to_string());
        env.insert(
            format!("{prefix}_HTTPCHALLENGE_ENTRYPOINT"),
            "web".to_string(),
        );
    }
    env
}

fn workload_request(
    name: &str,
    request: &CreateGatewayRequest,
    group_id: Uuid,
    env: HashMap<String, String>,
) -> Value {
    json!({
        "name": format!("gateway-{name}"),
        "service_name": format!("gateway-{name}"),
        "image": GATEWAY_IMAGE,
        "cpu_millicores": GATEWAY_CPU_MILLICORES,
        "memory_bytes": GATEWAY_MEMORY_BYTES,
        "disk_bytes": GATEWAY_DISK_BYTES,
        "env_vars": env,
        "ports": [
            { "container_port": 80, "protocol": "tcp", "node_port": request.http_port },
            { "container_port": 443, "protocol": "tcp", "node_port": request.https_port }
        ],
        "resource_group_id": group_id,
        "restart_policy": "always"
    })
}

fn agent_port() -> u16 {
    std::env::var("CSFX_AGENT_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_AGENT_PORT)
}

fn acme_settings() -> Option<(String, String)> {
    let resolver = std::env::var("INGRESS_CERT_RESOLVER")
        .ok()
        .filter(|value| !value.is_empty())?;
    let email = std::env::var("INGRESS_ACME_EMAIL")
        .ok()
        .filter(|value| !value.is_empty())?;
    Some((resolver, email))
}

pub async fn list_gateways(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<Vec<GatewayResponse>>, ApiError> {
    let group = ensure_group(&state, group_id).await?;
    let gateways = ingress_gateways::Entity::find()
        .filter(ingress_gateways::Column::ResourceGroupId.eq(group_id))
        .order_by_asc(ingress_gateways::Column::Name)
        .all(&state.db_conn)
        .await
        .map_err(internal)?;
    let statuses = workload_statuses(&state, &gateways).await?;
    Ok(Json(
        gateways
            .into_iter()
            .map(|gateway| {
                let status = statuses.get(&gateway.workload_id).cloned();
                response_for(gateway, &group.name, status)
            })
            .collect(),
    ))
}

async fn workload_statuses(
    state: &AppState,
    gateways: &[ingress_gateways::Model],
) -> Result<HashMap<Uuid, String>, ApiError> {
    let ids: Vec<Uuid> = gateways.iter().map(|gateway| gateway.workload_id).collect();
    Ok(workloads::Entity::find()
        .filter(workloads::Column::Id.is_in(ids))
        .all(&state.db_conn)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|workload| (workload.id, workload.status))
        .collect())
}

pub async fn create_gateway(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<CreateGatewayRequest>,
) -> Result<(StatusCode, Json<GatewayResponse>), ApiError> {
    let group = ensure_group(&state, group_id).await?;
    let name =
        validate_request(&request).map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    ensure_name_free(&state, group_id, &name).await?;

    let gateway_id = Uuid::new_v4();
    let workload_id = schedule_workload(&state, &name, &request, group_id, gateway_id).await?;
    let gateway = ingress_gateways::ActiveModel {
        id: Set(gateway_id),
        resource_group_id: Set(group_id),
        workload_id: Set(workload_id),
        name: Set(name),
        http_port: Set(i32::from(request.http_port)),
        https_port: Set(i32::from(request.https_port)),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await
    .map_err(internal)?;

    tracing::info!(
        user_id = %claims.user_id,
        gateway_id = %gateway.id,
        workload_id = %workload_id,
        "ingress gateway created"
    );
    Ok((
        StatusCode::CREATED,
        Json(response_for(gateway, &group.name, Some("pending".into()))),
    ))
}

async fn ensure_name_free(state: &AppState, group_id: Uuid, name: &str) -> Result<(), ApiError> {
    let taken = ingress_gateways::Entity::find()
        .filter(ingress_gateways::Column::ResourceGroupId.eq(group_id))
        .filter(ingress_gateways::Column::Name.eq(name))
        .count(&state.db_conn)
        .await
        .map_err(internal)?;
    if taken > 0 {
        return Err(error(StatusCode::CONFLICT, "gateway name is already used"));
    }
    Ok(())
}

async fn schedule_workload(
    state: &AppState,
    name: &str,
    request: &CreateGatewayRequest,
    group_id: Uuid,
    gateway_id: Uuid,
) -> Result<Uuid, ApiError> {
    let endpoint = provider_endpoint(agent_port(), group_id, gateway_id);
    let acme = acme_settings();
    let env = gateway_environment(
        &endpoint,
        acme.as_ref()
            .map(|(resolver, email)| (resolver.as_str(), email.as_str())),
    );
    let body = workload_request(name, request, group_id, env);
    let (status, response) = state
        .service_client
        .forward_to_scheduler(reqwest::Method::POST, "/workloads", Some(body), None)
        .await
        .map_err(|e| {
            error(
                StatusCode::BAD_GATEWAY,
                &format!("scheduler unavailable: {e}"),
            )
        })?;
    if !status.is_success() {
        let message = response
            .as_ref()
            .and_then(|body| body["error"].as_str())
            .unwrap_or("scheduler rejected the gateway workload");
        return Err(error(
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            message,
        ));
    }
    response
        .as_ref()
        .and_then(|body| body["workload_id"].as_str())
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| internal("scheduler response lacks workload_id"))
}

pub async fn delete_gateway(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, gateway_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    ensure_group(&state, group_id).await?;
    let gateway = find_gateway(&state, group_id, gateway_id).await?;
    let path = format!("/workloads/{}", gateway.workload_id);
    let (status, _) = state
        .service_client
        .forward_to_scheduler(reqwest::Method::DELETE, &path, None, None)
        .await
        .map_err(|e| {
            error(
                StatusCode::BAD_GATEWAY,
                &format!("scheduler unavailable: {e}"),
            )
        })?;
    if !status.is_success() && status.as_u16() != StatusCode::NOT_FOUND.as_u16() {
        return Err(error(
            StatusCode::BAD_GATEWAY,
            "scheduler failed to delete the gateway workload",
        ));
    }
    ingress_gateways::Entity::delete_by_id(gateway_id)
        .exec(&state.db_conn)
        .await
        .map_err(internal)?;
    zone_cert::remove(&state.db_conn, gateway_id)
        .await
        .map_err(internal)?;

    tracing::info!(user_id = %claims.user_id, gateway_id = %gateway_id, "ingress gateway deleted");
    Ok(StatusCode::NO_CONTENT)
}

async fn find_gateway(
    state: &AppState,
    group_id: Uuid,
    gateway_id: Uuid,
) -> Result<ingress_gateways::Model, ApiError> {
    ingress_gateways::Entity::find_by_id(gateway_id)
        .filter(ingress_gateways::Column::ResourceGroupId.eq(group_id))
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "gateway not found"))
}

pub async fn gateway_config(
    agent: AgentApiKey,
    State(state): State<AppState>,
    Path((group_id, gateway_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, ApiError> {
    let gateway = find_gateway(&state, group_id, gateway_id).await?;
    ensure_hosted_by(&state, &gateway, &agent).await?;
    let targets = ingress_sync::load_targets(&state.db_conn, gateway.id)
        .await
        .map_err(internal)?;
    let zone = dns_sync::zone_from_env();
    let resolver = acme_settings().map(|(resolver, _)| resolver);
    let mut config = ingress_sync::provider_config(&targets, &zone, resolver.as_deref());
    let sans = ingress_sync::zone_hostnames(&targets, &zone);
    let certificate = zone_cert::certificate_for(&state.db_conn, gateway.id, &sans)
        .await
        .map_err(internal)?;
    if let Some(certificate) = certificate {
        config["tls"] = zone_cert::tls_section(&certificate);
    }
    Ok(Json(config))
}

async fn ensure_hosted_by(
    state: &AppState,
    gateway: &ingress_gateways::Model,
    agent: &AgentApiKey,
) -> Result<(), ApiError> {
    let workload = workloads::Entity::find_by_id(gateway.workload_id)
        .one(&state.db_conn)
        .await
        .map_err(internal)?;
    match workload.and_then(|workload| workload.assigned_agent_id) {
        Some(agent_id) if agent_id == agent.agent_id => Ok(()),
        _ => Err(error(
            StatusCode::FORBIDDEN,
            "gateway is not hosted by this agent",
        )),
    }
}

pub fn ingress_gateways_router() -> Router<AppState> {
    Router::new()
        .route(
            "/resource-groups/{id}/gateways",
            get(list_gateways).post(create_gateway),
        )
        .route(
            "/resource-groups/{id}/gateways/{gateway_id}",
            delete(delete_gateway),
        )
        .route(
            "/resource-groups/{id}/gateways/{gateway_id}/config",
            get(gateway_config),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(name: &str, http_port: u16, https_port: u16) -> CreateGatewayRequest {
        CreateGatewayRequest {
            name: name.to_string(),
            http_port,
            https_port,
        }
    }

    #[test]
    fn name_is_normalized_and_validated() {
        assert_eq!(
            validate_request(&request(" Edge ", 80, 443)),
            Ok("edge".to_string())
        );
        for name in ["", "-x", "x_y", "a.b", &"a".repeat(41)] {
            assert!(validate_request(&request(name, 80, 443)).is_err(), "{name}");
        }
    }

    #[test]
    fn ports_must_be_nonzero_and_distinct() {
        assert!(validate_request(&request("edge", 0, 443)).is_err());
        assert!(validate_request(&request("edge", 8080, 8080)).is_err());
        assert!(validate_request(&request("edge", 8080, 8443)).is_ok());
    }

    #[test]
    fn endpoint_lowercases_the_group_name() {
        assert_eq!(
            endpoint_for("edge", "Prod", "csfx.internal"),
            "edge.prod.csfx.internal"
        );
    }

    #[test]
    fn environment_points_traefik_at_the_agent_endpoint() {
        let endpoint = provider_endpoint(7443, Uuid::nil(), Uuid::from_u128(1));
        let env = gateway_environment(&endpoint, Some(("letsencrypt", "ops@example.com")));
        assert_eq!(env["TRAEFIK_PROVIDERS_HTTP_ENDPOINT"], endpoint);
        assert!(endpoint.contains(":7443/ingress-config/"));
        assert_eq!(
            env["TRAEFIK_CERTIFICATESRESOLVERS_LETSENCRYPT_ACME_EMAIL"],
            "ops@example.com"
        );
        assert!(!gateway_environment(&endpoint, None)
            .keys()
            .any(|key| key.contains("CERTIFICATESRESOLVERS")));
    }

    #[test]
    fn workload_maps_requested_node_ports_to_traefik_entrypoints() {
        let body = workload_request(
            "edge",
            &request("edge", 8080, 8443),
            Uuid::nil(),
            HashMap::new(),
        );
        assert_eq!(body["ports"][0]["container_port"], 80);
        assert_eq!(body["ports"][0]["node_port"], 8080);
        assert_eq!(body["ports"][1]["container_port"], 443);
        assert_eq!(body["ports"][1]["node_port"], 8443);
        assert_eq!(body["restart_policy"], "always");
    }
}
