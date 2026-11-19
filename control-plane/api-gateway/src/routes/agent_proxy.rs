use std::sync::OnceLock;
use std::time::Duration;

use crate::routes::proxy::ProxyError;
use crate::service_client::Service;
use axum::{
    body::Body,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use entity::entities::{agents, workloads};
use futures_util::{SinkExt, StreamExt};
use sea_orm::EntityTrait;
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use uuid::Uuid;

use crate::{
    auth::jwt::{create_ticket, verify_ticket, EXEC_PURPOSE, NODE_METRICS_PURPOSE, VNC_PURPOSE},
    auth::rbac::{CanManageSystem, CanManageWorkloads, CanViewAgents, CanViewWorkloads},
    AppState,
};

const CSFX_AGENT_PORT_ENV: &str = "CSFX_AGENT_PORT";
const CSFX_AGENT_DIRECT_ENV: &str = "CSFX_AGENT_DIRECT_HOSTNAME";
const DEFAULT_AGENT_PORT: u16 = 7443;
const PROXY_TICKET_HEADER: &str = "X-Csfx-Proxy-Ticket";
const METRICS_TICKET_SCOPE: &str = "__node_metrics__";
const POWER_TICKET_SCOPE: &str = "__power__";
const DISKS_TICKET_SCOPE: &str = "__disks__";
const AGENT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| shared::http::client(AGENT_REQUEST_TIMEOUT))
}

fn error(status: StatusCode, message: impl Into<String>) -> ProxyError {
    (status, Json(json!({ "error": message.into() })))
}

fn bad_gateway(message: impl Into<String>) -> ProxyError {
    error(StatusCode::BAD_GATEWAY, message)
}

fn database_error(e: sea_orm::DbErr) -> ProxyError {
    tracing::error!(error = %e, "agent proxy database query failed");
    error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

pub(crate) fn agent_port() -> u16 {
    std::env::var(CSFX_AGENT_PORT_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_AGENT_PORT)
}

pub(crate) fn agent_url(scheme: &str, host: &str, path: &str) -> String {
    format!("{scheme}://{host}:{}{path}", agent_port())
}

pub(crate) async fn resolve_agent_tunnel_ip(
    state: &AppState,
    agent_id: Uuid,
) -> Result<String, ProxyError> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "agent not found"))?;

    if std::env::var(CSFX_AGENT_DIRECT_ENV).is_ok() {
        return Ok(agent.hostname);
    }

    agent.wg_tunnel_ip.ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent has no known tunnel address",
        )
    })
}

async fn resolve_agent_target(
    state: &AppState,
    workload_id: Uuid,
) -> Result<(String, Uuid), ProxyError> {
    let workload = workloads::Entity::find_by_id(workload_id)
        .one(&state.db_conn)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "workload not found"))?;

    let agent_id = workload
        .assigned_agent_id
        .ok_or_else(|| error(StatusCode::CONFLICT, "workload has no assigned agent"))?;

    Ok((resolve_agent_tunnel_ip(state, agent_id).await?, agent_id))
}

pub(crate) async fn fetch_proxy_ticket(
    state: &AppState,
    agent_id: Uuid,
    workload_id: &str,
) -> Result<String, ProxyError> {
    let (status, body) = state
        .service_client
        .call_json(
            Service::Registry,
            reqwest::Method::POST,
            "/internal/agent-proxy-ticket",
            Some(json!({ "agent_id": agent_id, "workload_id": workload_id })),
        )
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "failed to reach registry");
            bad_gateway("registry unreachable")
        })?;

    if !status.is_success() {
        return Err(bad_gateway("registry refused to issue proxy ticket"));
    }

    let body = body.ok_or_else(|| bad_gateway("registry returned empty proxy ticket response"))?;
    let payload = body["payload"]
        .as_str()
        .ok_or_else(|| bad_gateway("malformed proxy ticket payload"))?;
    let signature = body["signature"]
        .as_str()
        .ok_or_else(|| bad_gateway("malformed proxy ticket signature"))?;

    Ok(format!("{}.{}", payload, signature))
}

async fn agent_call(
    method: reqwest::Method,
    url: &str,
    ticket: String,
    subject: Uuid,
    refusal: &str,
    body: Option<serde_json::Value>,
) -> Result<reqwest::Response, ProxyError> {
    let mut request = client()
        .request(method, url)
        .header(PROXY_TICKET_HEADER, ticket);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let resp = request.send().await.map_err(|e| {
        tracing::warn!(error = %e, "failed to reach agent");
        bad_gateway("agent unreachable")
    })?;

    if resp.status().is_success() {
        return Ok(resp);
    }

    let agent_status = resp.status();
    let detail = resp.text().await.unwrap_or_default();
    tracing::warn!(subject = %subject, agent_status = %agent_status, body = %detail, "{refusal}");
    Err((
        StatusCode::BAD_GATEWAY,
        Json(json!({ "error": refusal, "detail": detail })),
    ))
}

async fn connect_agent_socket(
    url: String,
    proxy_ticket: String,
    subject: Uuid,
    label: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    ProxyError,
> {
    let mut request = url.into_client_request().map_err(|e| {
        tracing::error!(subject = %subject, error = %e, "invalid agent {label} url");
        error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    request.headers_mut().insert(
        PROXY_TICKET_HEADER,
        proxy_ticket.parse().map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid proxy ticket header",
            )
        })?,
    );

    let (socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| {
            tracing::warn!(subject = %subject, error = %e, "failed to connect to agent {label} socket");
            bad_gateway(format!("failed to connect to agent {label} socket"))
        })?;
    Ok(socket)
}

fn issue_ticket(
    purpose: &str,
    label: &str,
    subject: Uuid,
    user_id: Uuid,
) -> Result<impl IntoResponse, ProxyError> {
    let ticket = create_ticket(purpose, subject, user_id).map_err(|e| {
        tracing::error!(error = %e, "failed to issue {label} ticket");
        error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    Ok(Json(json!({ "ticket": ticket })))
}

fn check_ticket(token: &str, purpose: &str, label: &str, subject: Uuid) -> Result<(), ProxyError> {
    verify_ticket(token, purpose, subject).map_err(|_| {
        error(
            StatusCode::UNAUTHORIZED,
            format!("invalid or expired {label} ticket"),
        )
    })?;
    Ok(())
}

pub async fn stream_workload_logs(
    CanViewWorkloads(_claims): CanViewWorkloads,
    State(state): State<AppState>,
    Path(workload_id): Path<Uuid>,
) -> Result<impl IntoResponse, ProxyError> {
    let (tunnel_ip, agent_id) = resolve_agent_target(&state, workload_id).await?;
    let ticket = fetch_proxy_ticket(&state, agent_id, &workload_id.to_string()).await?;
    let url = agent_url("http", &tunnel_ip, &format!("/logs/{workload_id}"));

    let resp = agent_call(
        reqwest::Method::GET,
        &url,
        ticket,
        workload_id,
        "agent refused log stream request",
        None,
    )
    .await?;

    let stream =
        futures_util::stream::once(async { Ok::<_, reqwest::Error>(axum::body::Bytes::new()) })
            .chain(resp.bytes_stream());

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        Body::from_stream(stream),
    ))
}

pub async fn issue_exec_ticket(
    CanManageWorkloads(claims): CanManageWorkloads,
    Path(workload_id): Path<Uuid>,
) -> Result<impl IntoResponse, ProxyError> {
    issue_ticket(EXEC_PURPOSE, "exec", workload_id, claims.user_id)
}

#[derive(Deserialize)]
pub struct TicketQuery {
    ticket: String,
}

pub async fn exec_workload(
    State(state): State<AppState>,
    Path(workload_id): Path<Uuid>,
    Query(query): Query<TicketQuery>,
    ws: WebSocketUpgrade,
) -> Result<axum::response::Response, ProxyError> {
    check_ticket(&query.ticket, EXEC_PURPOSE, "exec", workload_id)?;
    let agent_socket = connect_workload_socket(&state, workload_id, "exec").await?;
    Ok(ws.on_upgrade(move |browser_socket| bridge_exec_sockets(browser_socket, agent_socket)))
}

async fn connect_workload_socket(
    state: &AppState,
    workload_id: Uuid,
    label: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    ProxyError,
> {
    let (tunnel_ip, agent_id) = resolve_agent_target(state, workload_id).await?;
    let proxy_ticket = fetch_proxy_ticket(state, agent_id, &workload_id.to_string()).await?;
    let url = agent_url("ws", &tunnel_ip, &format!("/{label}/{workload_id}"));
    connect_agent_socket(url, proxy_ticket, workload_id, label).await
}

async fn bridge_exec_sockets(
    browser_socket: WebSocket,
    agent_socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    let (mut browser_sink, mut browser_stream) = browser_socket.split();
    let (mut agent_sink, mut agent_stream) = agent_socket.split();

    let browser_to_agent = tokio::spawn(async move {
        while let Some(Ok(msg)) = browser_stream.next().await {
            let forwarded = match msg {
                Message::Binary(data) => TungsteniteMessage::Binary(data),
                Message::Text(text) => TungsteniteMessage::Text(text.as_str().into()),
                Message::Close(_) => break,
                _ => continue,
            };
            if agent_sink.send(forwarded).await.is_err() {
                break;
            }
        }
    });

    let agent_to_browser = tokio::spawn(async move {
        while let Some(Ok(msg)) = agent_stream.next().await {
            let forwarded = match msg {
                TungsteniteMessage::Binary(data) => Message::Binary(data),
                TungsteniteMessage::Text(text) => Message::Text(text.as_str().into()),
                TungsteniteMessage::Close(_) => break,
                _ => continue,
            };
            if browser_sink.send(forwarded).await.is_err() {
                break;
            }
        }
    });

    tokio::select! {
        _ = browser_to_agent => {}
        _ = agent_to_browser => {}
    }
}

pub async fn issue_vnc_ticket(
    CanManageWorkloads(claims): CanManageWorkloads,
    Path(workload_id): Path<Uuid>,
) -> Result<impl IntoResponse, ProxyError> {
    issue_ticket(VNC_PURPOSE, "vnc", workload_id, claims.user_id)
}

pub async fn vnc_workload(
    State(state): State<AppState>,
    Path(workload_id): Path<Uuid>,
    Query(query): Query<TicketQuery>,
    ws: WebSocketUpgrade,
) -> Result<axum::response::Response, ProxyError> {
    check_ticket(&query.ticket, VNC_PURPOSE, "vnc", workload_id)?;
    let agent_socket = connect_workload_socket(&state, workload_id, "vnc").await?;
    Ok(ws.on_upgrade(move |browser_socket| bridge_exec_sockets(browser_socket, agent_socket)))
}

pub async fn issue_node_metrics_ticket(
    CanViewAgents(claims): CanViewAgents,
    Path(agent_id): Path<Uuid>,
) -> Result<impl IntoResponse, ProxyError> {
    issue_ticket(
        NODE_METRICS_PURPOSE,
        "node metrics",
        agent_id,
        claims.user_id,
    )
}

pub async fn stream_node_metrics(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Query(query): Query<TicketQuery>,
    ws: WebSocketUpgrade,
) -> Result<axum::response::Response, ProxyError> {
    check_ticket(
        &query.ticket,
        NODE_METRICS_PURPOSE,
        "node metrics",
        agent_id,
    )?;

    let tunnel_ip = resolve_agent_tunnel_ip(&state, agent_id).await?;
    let proxy_ticket = fetch_proxy_ticket(&state, agent_id, METRICS_TICKET_SCOPE).await?;
    let url = agent_url("ws", &tunnel_ip, "/metrics/stream");
    let agent_socket = connect_agent_socket(url, proxy_ticket, agent_id, "metrics").await?;

    Ok(ws.on_upgrade(move |browser_socket| bridge_metrics_socket(browser_socket, agent_socket)))
}

async fn bridge_metrics_socket(
    browser_socket: WebSocket,
    agent_socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    let (mut browser_sink, _browser_stream) = browser_socket.split();
    let (_agent_sink, mut agent_stream) = agent_socket.split();

    while let Some(Ok(msg)) = agent_stream.next().await {
        let forwarded = match msg {
            TungsteniteMessage::Text(text) => Message::Text(text.as_str().into()),
            TungsteniteMessage::Close(_) => break,
            _ => continue,
        };
        if browser_sink.send(forwarded).await.is_err() {
            break;
        }
    }
}

#[derive(Deserialize)]
pub struct PowerRequest {
    action: PowerAction,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum PowerAction {
    Reboot,
    Poweroff,
}

impl PowerAction {
    fn as_str(&self) -> &'static str {
        match self {
            PowerAction::Reboot => "reboot",
            PowerAction::Poweroff => "poweroff",
        }
    }
}

pub async fn power_agent(
    CanManageSystem(claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<PowerRequest>,
) -> Result<impl IntoResponse, ProxyError> {
    tracing::warn!(
        user_id = %claims.user_id,
        agent_id = %agent_id,
        action = req.action.as_str(),
        "power action requested"
    );

    let tunnel_ip = resolve_agent_tunnel_ip(&state, agent_id).await?;
    let proxy_ticket = fetch_proxy_ticket(&state, agent_id, POWER_TICKET_SCOPE).await?;
    let url = agent_url("http", &tunnel_ip, "/power");

    agent_call(
        reqwest::Method::POST,
        &url,
        proxy_ticket,
        agent_id,
        "agent refused power action",
        Some(json!({ "action": req.action.as_str() })),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_agent_disks(
    CanViewAgents(_claims): CanViewAgents,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<impl IntoResponse, ProxyError> {
    let tunnel_ip = resolve_agent_tunnel_ip(&state, agent_id).await?;
    let proxy_ticket = fetch_proxy_ticket(&state, agent_id, DISKS_TICKET_SCOPE).await?;
    let url = agent_url("http", &tunnel_ip, "/disks");

    let resp = agent_call(
        reqwest::Method::GET,
        &url,
        proxy_ticket,
        agent_id,
        "agent refused disks request",
        None,
    )
    .await?;

    let body: serde_json::Value = resp.json().await.map_err(|e| {
        tracing::warn!(error = %e, "invalid agent response");
        bad_gateway("invalid agent response")
    })?;

    Ok(Json(body))
}

pub fn agent_proxy_routes() -> Router<AppState> {
    Router::new()
        .route("/workloads/{id}/logs", get(stream_workload_logs))
        .route(
            "/workloads/{id}/exec/ticket",
            axum::routing::post(issue_exec_ticket),
        )
        .route("/workloads/{id}/exec", get(exec_workload))
        .route(
            "/workloads/{id}/vnc/ticket",
            axum::routing::post(issue_vnc_ticket),
        )
        .route("/workloads/{id}/vnc", get(vnc_workload))
        .route(
            "/agents/{id}/metrics/ticket",
            axum::routing::post(issue_node_metrics_ticket),
        )
        .route("/agents/{id}/metrics/stream", get(stream_node_metrics))
        .route("/agents/{id}/power", axum::routing::post(power_agent))
        .route("/agents/{id}/disks", get(get_agent_disks))
}
