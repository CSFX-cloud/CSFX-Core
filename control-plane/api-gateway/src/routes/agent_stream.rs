use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{auth::agent::AgentApiKey, AppState};

const ASSIGNMENT_SIGNAL: &str = "assignment";

pub(crate) fn is_internal_source(addr: &SocketAddr) -> bool {
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

#[derive(Clone, Default)]
pub struct AgentStreamRegistry {
    senders: Arc<Mutex<HashMap<Uuid, mpsc::UnboundedSender<Message>>>>,
}

impl AgentStreamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    async fn register(&self, agent_id: Uuid) -> mpsc::UnboundedReceiver<Message> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.senders.lock().await.insert(agent_id, tx);
        rx
    }

    async fn unregister(&self, agent_id: Uuid) {
        self.senders.lock().await.remove(&agent_id);
    }

    pub async fn notify_assignment(&self, agent_id: Uuid) -> bool {
        let senders = self.senders.lock().await;
        match senders.get(&agent_id) {
            Some(tx) => tx.send(Message::Text(ASSIGNMENT_SIGNAL.into())).is_ok(),
            None => false,
        }
    }
}

pub async fn agent_stream_handler(
    agent: AgentApiKey,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_agent_stream(socket, state, agent.agent_id))
}

async fn handle_agent_stream(socket: WebSocket, state: AppState, agent_id: Uuid) {
    info!(agent_id = %agent_id, "agent stream connected");
    let mut rx = state.agent_stream_registry.register(agent_id).await;
    let (mut sink, mut stream) = socket.split();

    loop {
        tokio::select! {
            outbound = rx.recv() => {
                match outbound {
                    Some(msg) => {
                        if sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            inbound = stream.next() => {
                match inbound {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => {
                        warn!(agent_id = %agent_id, error = %e, "agent stream read error");
                        break;
                    }
                }
            }
        }
    }

    state.agent_stream_registry.unregister(agent_id).await;
    info!(agent_id = %agent_id, "agent stream disconnected");
}

pub async fn notify_assignment_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected notify-assignment from non-internal source");
        return Err(StatusCode::FORBIDDEN);
    }

    let delivered = state
        .agent_stream_registry
        .notify_assignment(agent_id)
        .await;
    info!(agent_id = %agent_id, delivered, "assignment notification processed");
    Ok(StatusCode::NO_CONTENT)
}

pub fn agent_stream_routes() -> Router<AppState> {
    Router::new().route("/internal/agent-stream", get(agent_stream_handler))
}

pub fn agent_stream_internal_routes() -> Router<AppState> {
    Router::new().route(
        "/internal/agents/{agent_id}/notify-assignment",
        post(notify_assignment_handler),
    )
}
