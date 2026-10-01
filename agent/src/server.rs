use anyhow::{anyhow, Context, Result};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures_util::{SinkExt, StreamExt};
use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_ASN1};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::pki::AgentPki;
use crate::runtime::Runtime;
use crate::system::LiveMetricsCollector;

const TICKET_HEADER: &str = "X-Csfx-Proxy-Ticket";
pub const METRICS_TICKET_SCOPE: &str = "__node_metrics__";
pub const POWER_TICKET_SCOPE: &str = "__power__";
pub const DISKS_TICKET_SCOPE: &str = "__disks__";

#[derive(Clone)]
pub struct ServerState {
    pub firecracker: Arc<crate::firecracker::runtime::FirecrackerRuntime>,
    pub qemu: Arc<crate::qemu::runtime::QemuRuntime>,
    pub running_containers: Arc<Mutex<HashMap<String, String>>>,
    pub agent_id: uuid::Uuid,
}

pub async fn run(state: ServerState, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/logs/{workload_id}", get(logs_handler))
        .route("/exec/{workload_id}", get(exec_handler))
        .route("/vnc/{workload_id}", get(vnc_handler))
        .route("/metrics/stream", get(metrics_stream_handler))
        .route("/power", post(power_handler))
        .route("/disks", get(disks_handler))
        .with_state(state);

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind agent inbound server")?;

    info!(port = port, "csfx-agent inbound server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("Agent inbound server stopped unexpectedly")
}

fn is_internal_source(addr: &SocketAddr) -> bool {
    let ip = addr.ip();
    ip.is_loopback()
        || match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                octets[0] == 10
                    || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
                    || (octets[0] == 192 && octets[1] == 168)
            }
            IpAddr::V6(_) => false,
        }
}

struct TicketClaims {
    agent_id: String,
    workload_id: String,
    expires_at: i64,
}

fn verify_ticket(headers: &HeaderMap, workload_id: &str, expected_agent_id: &str) -> Result<()> {
    let raw = headers
        .get(TICKET_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("missing {} header", TICKET_HEADER))?;

    let (payload_b64, signature_b64) = raw
        .split_once('.')
        .ok_or_else(|| anyhow!("malformed proxy ticket"))?;

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .context("invalid ticket payload encoding")?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .context("invalid ticket signature encoding")?;

    let ca_pem = AgentPki::load_ca_pem().context("failed to load trusted CA certificate")?;
    let public_key_bytes = extract_ca_public_key(&ca_pem)?;

    let verifier = UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &public_key_bytes);
    verifier
        .verify(&payload_bytes, &signature)
        .map_err(|_| anyhow!("proxy ticket signature verification failed"))?;

    let claims = parse_claims(&payload_bytes)?;

    if claims.workload_id != workload_id {
        return Err(anyhow!("proxy ticket workload_id mismatch"));
    }
    if claims.agent_id != expected_agent_id {
        return Err(anyhow!("proxy ticket agent_id mismatch"));
    }
    if claims.expires_at < chrono::Utc::now().timestamp() {
        return Err(anyhow!("proxy ticket expired"));
    }

    Ok(())
}

fn parse_claims(payload_bytes: &[u8]) -> Result<TicketClaims> {
    let payload = std::str::from_utf8(payload_bytes).context("proxy ticket payload not utf8")?;
    let mut parts = payload.splitn(3, '.');

    let agent_id = parts
        .next()
        .ok_or_else(|| anyhow!("proxy ticket missing agent_id"))?
        .to_string();
    let workload_id = parts
        .next()
        .ok_or_else(|| anyhow!("proxy ticket missing workload_id"))?
        .to_string();
    let expires_at = parts
        .next()
        .ok_or_else(|| anyhow!("proxy ticket missing expiry"))?
        .parse::<i64>()
        .context("proxy ticket expiry not a valid timestamp")?;

    Ok(TicketClaims {
        agent_id,
        workload_id,
        expires_at,
    })
}

fn extract_ca_public_key(ca_pem: &str) -> Result<Vec<u8>> {
    let (_, pem) =
        x509_parser::pem::parse_x509_pem(ca_pem.as_bytes()).context("failed to parse CA PEM")?;
    let cert = pem.parse_x509().context("failed to parse CA certificate")?;
    Ok(cert
        .tbs_certificate
        .subject_pki
        .subject_public_key
        .data
        .to_vec())
}

async fn logs_handler(
    State(state): State<ServerState>,
    Path(workload_id): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(workload_id = %workload_id, source = %addr, "log stream request received");

    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected agent inbound request from non-internal source");
        return Err((StatusCode::FORBIDDEN, "source not allowed".to_string()));
    }

    verify_ticket(&headers, &workload_id, &state.agent_id.to_string()).map_err(|e| {
        warn!(workload_id = %workload_id, error = %e, "rejected request with invalid proxy ticket");
        (StatusCode::UNAUTHORIZED, e.to_string())
    })?;

    let container_id = state
        .running_containers
        .lock()
        .await
        .get(&workload_id)
        .cloned()
        .ok_or_else(|| {
            warn!(workload_id = %workload_id, "log stream requested for workload not running here");
            (
                StatusCode::NOT_FOUND,
                "workload not running here".to_string(),
            )
        })?;

    info!(workload_id = %workload_id, container_id = %container_id, "opening log stream to guest");
    let stream =
        futures_util::stream::once(async { Ok::<_, std::io::Error>(axum::body::Bytes::new()) })
            .chain(state.firecracker.logs(&container_id));

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        axum::body::Body::from_stream(stream),
    ))
}

async fn exec_handler(
    State(state): State<ServerState>,
    Path(workload_id): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<axum::response::Response, (StatusCode, String)> {
    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected agent inbound request from non-internal source");
        return Err((StatusCode::FORBIDDEN, "source not allowed".to_string()));
    }

    verify_ticket(&headers, &workload_id, &state.agent_id.to_string()).map_err(|e| {
        warn!(workload_id = %workload_id, error = %e, "rejected request with invalid proxy ticket");
        (StatusCode::UNAUTHORIZED, e.to_string())
    })?;

    let container_id = state
        .running_containers
        .lock()
        .await
        .get(&workload_id)
        .cloned()
        .ok_or((
            StatusCode::NOT_FOUND,
            "workload not running here".to_string(),
        ))?;

    Ok(ws.on_upgrade(move |socket| handle_exec_socket(socket, state, container_id)))
}

async fn handle_exec_socket(socket: WebSocket, state: ServerState, container_id: String) {
    let exec_session = match state.firecracker.exec(&container_id).await {
        Ok(session) => session,
        Err(e) => {
            warn!(container_id = %container_id, error = %e, "failed to start exec session");
            return;
        }
    };

    let mut output = exec_session.output;
    let mut input = exec_session.input;

    let (mut ws_sink, mut ws_stream) = socket.split();

    let output_task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match output.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if ws_sink
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let input_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Binary(data) => {
                    if input.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Message::Text(text) => {
                    if input.write_all(text.as_bytes()).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = output_task => {}
        _ = input_task => {}
    }

    info!(container_id = %container_id, "exec session closed");
}

async fn vnc_handler(
    State(state): State<ServerState>,
    Path(workload_id): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<axum::response::Response, (StatusCode, String)> {
    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected agent inbound request from non-internal source");
        return Err((StatusCode::FORBIDDEN, "source not allowed".to_string()));
    }

    verify_ticket(&headers, &workload_id, &state.agent_id.to_string()).map_err(|e| {
        warn!(workload_id = %workload_id, error = %e, "rejected request with invalid proxy ticket");
        (StatusCode::UNAUTHORIZED, e.to_string())
    })?;

    let vnc_socket_path = state
        .qemu
        .vnc_socket_path(&workload_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "vm not running here".to_string()))?;

    Ok(ws.on_upgrade(move |socket| handle_vnc_socket(socket, workload_id, vnc_socket_path)))
}

async fn handle_vnc_socket(
    socket: WebSocket,
    workload_id: String,
    vnc_socket_path: std::path::PathBuf,
) {
    let stream = match tokio::net::UnixStream::connect(&vnc_socket_path).await {
        Ok(stream) => stream,
        Err(e) => {
            warn!(workload_id = %workload_id, error = %e, "failed to connect to vnc socket");
            return;
        }
    };

    let (mut vnc_read, mut vnc_write) = stream.into_split();
    let (mut ws_sink, mut ws_stream) = socket.split();

    let output_task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match vnc_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if ws_sink
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let input_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Binary(data) => {
                    if vnc_write.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = output_task => {}
        _ = input_task => {}
    }

    info!(workload_id = %workload_id, "vnc session closed");
}

async fn metrics_stream_handler(
    State(state): State<ServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<axum::response::Response, (StatusCode, String)> {
    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected agent inbound request from non-internal source");
        return Err((StatusCode::FORBIDDEN, "source not allowed".to_string()));
    }

    verify_ticket(&headers, METRICS_TICKET_SCOPE, &state.agent_id.to_string())
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(ws.on_upgrade(handle_metrics_socket))
}

async fn handle_metrics_socket(socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let mut collector = LiveMetricsCollector::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let metrics = collector.sample();
                let payload = match serde_json::to_string(&metrics) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!(error = %e, "failed to serialize live metrics");
                        continue;
                    }
                };
                if sink.send(Message::Text(payload.into())).await.is_err() {
                    break;
                }
            }
            msg = stream.next() => match msg {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(_)) => break,
                _ => {}
            }
        }
    }

    info!("live metrics session closed");
}

#[derive(Debug, Deserialize)]
struct PowerRequest {
    action: PowerAction,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PowerAction {
    Reboot,
    Poweroff,
}

impl PowerAction {
    fn systemctl_verb(&self) -> &'static str {
        match self {
            PowerAction::Reboot => "reboot",
            PowerAction::Poweroff => "poweroff",
        }
    }
}

async fn disks_handler(
    State(state): State<ServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::disks::DiskInfo>>, (StatusCode, String)> {
    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected agent inbound request from non-internal source");
        return Err((StatusCode::FORBIDDEN, "source not allowed".to_string()));
    }

    verify_ticket(&headers, DISKS_TICKET_SCOPE, &state.agent_id.to_string())
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(Json(crate::disks::collect_disks().await))
}

async fn power_handler(
    State(state): State<ServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<PowerRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected agent inbound request from non-internal source");
        return Err((StatusCode::FORBIDDEN, "source not allowed".to_string()));
    }

    verify_ticket(&headers, POWER_TICKET_SCOPE, &state.agent_id.to_string())
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let verb = req.action.systemctl_verb();
    info!(agent_id = %state.agent_id, action = verb, "power action requested");

    let output = Command::new("systemctl")
        .arg(verb)
        .output()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        warn!(action = verb, error = %stderr, "power action failed");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, stderr));
    }

    Ok(StatusCode::NO_CONTENT)
}
