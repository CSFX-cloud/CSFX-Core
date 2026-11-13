use axum::{
    body::Body,
    extract::{OriginalUri, Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json},
    routing::any,
    Router,
};
use entity::entities::{agents, buckets, garage_nodes};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use crate::ttl_cache::TtlCache;
use crate::AppState;

const S3_PORT: u16 = 3900;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const BUCKET_CACHE_TTL: Duration = Duration::from_secs(30);
const NODE_CACHE_TTL: Duration = Duration::from_secs(5);
const NODE_CACHE_KEY: &str = "up";

type ProxyError = (StatusCode, Json<serde_json::Value>);

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static BUCKET_EXPOSURE: LazyLock<TtlCache<String>> =
    LazyLock::new(|| TtlCache::new(BUCKET_CACHE_TTL));
static NODE_HOSTS: LazyLock<TtlCache<Vec<String>>> =
    LazyLock::new(|| TtlCache::new(NODE_CACHE_TTL));
static NEXT_NODE: AtomicUsize = AtomicUsize::new(0);

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .pool_idle_timeout(POOL_IDLE_TIMEOUT)
            .build()
            .expect("failed to build s3 proxy client")
    })
}

fn error(status: StatusCode, message: &str) -> ProxyError {
    (status, Json(json!({ "error": message })))
}

fn database_error(e: sea_orm::DbErr) -> ProxyError {
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("database error: {}", e),
    )
}

fn bad_gateway(e: &reqwest::Error) -> ProxyError {
    error(
        StatusCode::BAD_GATEWAY,
        &format!("failed to reach garage node: {}", e),
    )
}

fn rotation(len: usize, start: usize) -> impl Iterator<Item = usize> {
    (0..len).map(move |offset| start.wrapping_add(offset) % len)
}

async fn bucket_exposure(state: &AppState, global_alias: &str) -> Result<String, ProxyError> {
    if let Some(exposure) = BUCKET_EXPOSURE.get(global_alias) {
        return Ok(exposure);
    }
    let bucket = buckets::Entity::find()
        .filter(buckets::Column::GlobalAlias.eq(global_alias))
        .one(&state.db_conn)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "bucket not found"))?;
    BUCKET_EXPOSURE.insert(global_alias, bucket.exposure.clone());
    Ok(bucket.exposure)
}

async fn node_host(state: &AppState, node: garage_nodes::Model) -> Option<String> {
    let Some(agent_id) = node.agent_id else {
        return Some(
            std::env::var("GARAGE_INTERNAL_HOST").unwrap_or_else(|_| "garage".to_string()),
        );
    };
    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .ok()
        .flatten()?;
    agent.wg_tunnel_ip
}

async fn healthy_hosts(state: &AppState) -> Result<Vec<String>, ProxyError> {
    if let Some(hosts) = NODE_HOSTS.get(NODE_CACHE_KEY) {
        return Ok(hosts);
    }
    let nodes = garage_nodes::Entity::find()
        .filter(garage_nodes::Column::Status.eq("up"))
        .all(&state.db_conn)
        .await
        .map_err(database_error)?;
    let mut hosts = Vec::with_capacity(nodes.len());
    for node in nodes {
        hosts.extend(node_host(state, node).await);
    }
    if hosts.is_empty() {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no garage node available",
        ));
    }
    NODE_HOSTS.insert(NODE_CACHE_KEY, hosts.clone());
    Ok(hosts)
}

async fn send_to_host(
    host: &str,
    raw_path_and_query: &str,
    method: &reqwest::Method,
    headers: &HeaderMap,
    body: Option<Body>,
) -> Result<reqwest::Response, reqwest::Error> {
    let url = format!("http://{}:{}{}", host, S3_PORT, raw_path_and_query);
    let mut request = client().request(method.clone(), &url);
    if let Some(body) = body {
        request = request.body(reqwest::Body::wrap_stream(body.into_data_stream()));
    }
    for (key, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            request = request.header(key.as_str(), value_str);
        }
    }
    request.send().await
}

async fn send_with_failover(
    hosts: &[String],
    raw_path_and_query: &str,
    method: Method,
    headers: &HeaderMap,
    body: Body,
) -> Result<reqwest::Response, ProxyError> {
    let reqwest_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
    let idempotent = matches!(method, Method::GET | Method::HEAD);
    let attempts = if idempotent { hosts.len().min(2) } else { 1 };
    let start = NEXT_NODE.fetch_add(1, Ordering::Relaxed);
    let mut body = Some(body).filter(|_| !idempotent);
    let mut last_error = None;
    for index in rotation(hosts.len(), start).take(attempts) {
        let host = &hosts[index];
        let result = send_to_host(
            host,
            raw_path_and_query,
            &reqwest_method,
            headers,
            body.take(),
        )
        .await;
        match result {
            Ok(response) => return Ok(response),
            Err(e) if e.is_connect() => {
                tracing::warn!(host = %host, error = %e, "garage node unreachable");
                last_error = Some(e);
            }
            Err(e) => return Err(bad_gateway(&e)),
        }
    }
    Err(last_error.map_or_else(
        || error(StatusCode::SERVICE_UNAVAILABLE, "no garage node available"),
        |e| bad_gateway(&e),
    ))
}

async fn proxy(
    state: &AppState,
    bucket: &str,
    raw_path_and_query: &str,
    require_external: bool,
    method: Method,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, ProxyError> {
    let exposure = bucket_exposure(state, bucket).await?;
    if require_external && exposure != "external" {
        return Err(error(StatusCode::NOT_FOUND, "bucket not found"));
    }
    let hosts = healthy_hosts(state).await?;
    let response = send_with_failover(&hosts, raw_path_and_query, method, &headers, body).await?;

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response_headers = HeaderMap::new();
    for (key, value) in response.headers().iter() {
        response_headers.insert(key.clone(), value.clone());
    }

    Ok((
        status,
        response_headers,
        Body::from_stream(response.bytes_stream()),
    ))
}

pub async fn proxy_s3_request(
    State(state): State<AppState>,
    Path((bucket, _path)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, ProxyError> {
    let raw = uri
        .path()
        .strip_prefix("/api/s3")
        .unwrap_or_else(|| uri.path());
    let raw_path_and_query = match uri.query() {
        Some(query) => format!("{}?{}", raw, query),
        None => raw.to_string(),
    };

    proxy(
        &state,
        &bucket,
        &raw_path_and_query,
        true,
        method,
        headers,
        body,
    )
    .await
}

pub async fn proxy_object_data(
    State(state): State<AppState>,
    Path((bucket, _key)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, ProxyError> {
    let raw = uri
        .path()
        .strip_prefix("/s3data")
        .unwrap_or_else(|| uri.path());
    let raw_path_and_query = match uri.query() {
        Some(query) => format!("{}?{}", raw, query),
        None => raw.to_string(),
    };

    proxy(
        &state,
        &bucket,
        &raw_path_and_query,
        false,
        method,
        headers,
        body,
    )
    .await
}

pub fn s3_proxy_routes() -> Router<AppState> {
    Router::new().route("/s3/{bucket}/{*path}", any(proxy_s3_request))
}

pub fn object_data_router() -> Router<AppState> {
    Router::new().route("/s3data/{bucket}/{*key}", any(proxy_object_data))
}

#[cfg(test)]
mod tests {
    use super::rotation;

    #[test]
    fn rotation_starts_at_offset_and_wraps() {
        assert_eq!(rotation(3, 0).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(rotation(3, 4).collect::<Vec<_>>(), vec![1, 2, 0]);
    }

    #[test]
    fn rotation_survives_counter_overflow() {
        assert_eq!(rotation(3, usize::MAX).count(), 3);
    }

    #[test]
    fn successive_starts_cover_every_node() {
        let firsts: Vec<_> = (0..6).map(|n| rotation(3, n).next().unwrap()).collect();
        assert_eq!(firsts, vec![0, 1, 2, 0, 1, 2]);
    }
}
