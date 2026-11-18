use axum::{
    body::Bytes,
    extract::{RawPathParams, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
    routing::{get, on, MethodFilter},
    Router,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageSystem, CanViewAgents},
    routes::proxy::{proxy, ProxyError},
    service_client::Service,
    AppState,
};

const GATEWAY_PREFIX: &str = "/registry";

#[derive(Clone, Copy)]
enum Access {
    Manage,
    View,
    Open,
}

const ROUTES: [(Method, &str, Access); 18] = [
    (Method::POST, "/admin/agents/pre-register", Access::Manage),
    (Method::GET, "/admin/agents/pending", Access::Manage),
    (Method::DELETE, "/admin/agents/pending/{id}", Access::Manage),
    (Method::POST, "/admin/bootstrap-tokens", Access::Manage),
    (Method::GET, "/admin/bootstrap-tokens", Access::Manage),
    (
        Method::POST,
        "/admin/bootstrap-tokens/{id}/revoke",
        Access::Manage,
    ),
    (Method::GET, "/admin/tokens", Access::Manage),
    (Method::GET, "/admin/agents", Access::View),
    (Method::GET, "/admin/agents/{id}", Access::View),
    (Method::DELETE, "/admin/agents/{id}", Access::Manage),
    (Method::GET, "/admin/statistics", Access::View),
    (Method::POST, "/admin/agents/{id}/revoke", Access::Manage),
    (Method::GET, "/admin/agents/{id}/endpoint", Access::View),
    (Method::GET, "/pki/crl", Access::Open),
    (Method::POST, "/agents/register", Access::Open),
    (Method::POST, "/agents/{id}/heartbeat", Access::Open),
    (Method::POST, "/agents/{id}/certificate", Access::Open),
    (
        Method::POST,
        "/agents/{id}/rotate-certificate",
        Access::Open,
    ),
];

async fn passthrough(
    state: &AppState,
    method: Method,
    uri: &Uri,
    params: RawPathParams,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    if params
        .iter()
        .any(|(_, value)| Uuid::parse_str(value).is_err())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "path identifiers must be UUIDs" })),
        ));
    }
    let path = uri
        .path_and_query()
        .map_or(uri.path(), |value| value.as_str());
    let upstream = path.strip_prefix(GATEWAY_PREFIX).unwrap_or(path);
    proxy(state, Service::Registry, method, upstream, headers, body).await
}

async fn manage(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    params: RawPathParams,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    passthrough(&state, method, &uri, params, &headers, body).await
}

async fn view(
    CanViewAgents(_claims): CanViewAgents,
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    params: RawPathParams,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    passthrough(&state, method, &uri, params, &headers, body).await
}

async fn open(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    params: RawPathParams,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    passthrough(&state, method, &uri, params, &headers, body).await
}

pub async fn registry_health(State(state): State<AppState>) -> impl IntoResponse {
    if state.service_client.check_registry_health().await {
        (StatusCode::OK, Json(json!({"status": "healthy"})))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "unavailable"})),
        )
    }
}

pub fn registry_routes() -> Router<AppState> {
    let router = ROUTES
        .iter()
        .fold(Router::new(), |router, (method, path, access)| {
            let filter = MethodFilter::try_from(method.clone()).expect("supported method");
            let route = match access {
                Access::Manage => on(filter, manage),
                Access::View => on(filter, view),
                Access::Open => on(filter, open),
            };
            router.route(&format!("{GATEWAY_PREFIX}{path}"), route)
        });
    router.route("/registry/health", get(registry_health))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY_SERVER: &str = include_str!("../../../registry/src/server.rs");

    fn normalized(path: &str) -> String {
        let mut result = String::new();
        let mut depth = 0;
        for character in path.chars() {
            match character {
                '{' => {
                    depth += 1;
                    result.push_str("{}");
                }
                '}' => depth -= 1,
                _ if depth == 0 => result.push(character),
                _ => {}
            }
        }
        result
    }

    fn registry_routes_declared() -> Vec<(String, &'static str)> {
        let mut declared = Vec::new();
        for chunk in REGISTRY_SERVER.split(".route(").skip(1) {
            let path = chunk.split('"').nth(1).expect("route path");
            let rest = chunk.split('"').skip(2).collect::<Vec<_>>().join("\"");
            for method in ["get", "post", "put", "patch", "delete"] {
                if rest.contains(&format!(" {method}(")) || rest.contains(&format!("\n{method}(")) {
                    declared.push((normalized(path), method));
                }
            }
        }
        declared
    }

    #[test]
    fn every_gateway_route_targets_an_existing_registry_route() {
        let declared = registry_routes_declared();
        for (method, path, _) in ROUTES.iter() {
            let wanted = (normalized(path), method.as_str().to_ascii_lowercase());
            assert!(
                declared.iter().any(|(declared_path, declared_method)| {
                    *declared_path == wanted.0 && *declared_method == wanted.1
                }),
                "registry has no route for {method} {path}"
            );
        }
    }
}
