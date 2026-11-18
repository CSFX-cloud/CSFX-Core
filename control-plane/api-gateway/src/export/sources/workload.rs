use entity::{Agents, Workloads};
use reqwest::Method;
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use std::{sync::OnceLock, time::Duration};
use uuid::Uuid;

use super::{request_body, response_stream, upstream};
use crate::{
    export::{
        engine::{Exporter, Reader},
        ExportError,
    },
    routes::agent_proxy::{fetch_proxy_ticket, resolve_agent_tunnel_ip},
    AppState,
};

const EXPORT_SCOPE_PREFIX: &str = "__export__";
const TICKET_HEADER: &str = "X-Csfx-Proxy-Ticket";
const VM_RUNTIME_CLASS: &str = "vm";
const AGENT_PORT_VARIABLE: &str = "CSFX_AGENT_PORT";
const DEFAULT_AGENT_PORT: u16 = 7443;
const READ_TIMEOUT: Duration = Duration::from_secs(300);

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(READ_TIMEOUT)
            .build()
            .expect("failed to build agent transfer client")
    })
}

fn row_name(workload_id: Uuid) -> String {
    format!("workloads/{workload_id}.json")
}

fn disk_name(workload_id: Uuid) -> String {
    format!("workloads/{workload_id}/boot.img")
}

fn gateway_failure((status, body): (axum::http::StatusCode, axum::Json<Value>)) -> ExportError {
    upstream(format!("{status} {}", body.0))
}

async fn agent_request(
    state: &AppState,
    agent_id: Uuid,
    workload_id: Uuid,
    method: Method,
    body: Option<reqwest::Body>,
) -> Result<reqwest::Response, ExportError> {
    let address = resolve_agent_tunnel_ip(state, agent_id)
        .await
        .map_err(gateway_failure)?;
    let scope = format!("{EXPORT_SCOPE_PREFIX}{workload_id}");
    let ticket = fetch_proxy_ticket(state, agent_id, &scope)
        .await
        .map_err(gateway_failure)?;
    let port = std::env::var(AGENT_PORT_VARIABLE)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_AGENT_PORT);
    let url = format!("http://{address}:{port}/export/{workload_id}/boot");
    let mut request = client().request(method, url).header(TICKET_HEADER, ticket);
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request.send().await.map_err(upstream)?;
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let detail = response.text().await.unwrap_or_default();
    Err(upstream(format!("agent answered {status}: {detail}")))
}

pub async fn export(
    state: &AppState,
    exporter: &mut Exporter,
    workload_id: Uuid,
) -> Result<(), ExportError> {
    let workload = Workloads::find_by_id(workload_id)
        .one(&state.db_conn)
        .await?
        .ok_or_else(|| ExportError::ResourceNotFound(format!("workload {workload_id}")))?;
    exporter
        .add_bytes(
            row_name(workload_id),
            "row",
            Some(workload_id),
            serde_json::to_vec(&workload)?,
        )
        .await?;
    let agent_id = match (workload.runtime_class.as_str(), workload.assigned_agent_id) {
        (VM_RUNTIME_CLASS, Some(agent_id)) => agent_id,
        _ => return Ok(()),
    };
    let response = agent_request(state, agent_id, workload_id, Method::GET, None).await?;
    exporter
        .add_stream(
            disk_name(workload_id),
            "disk",
            Some(workload_id),
            response_stream(response),
        )
        .await
}

async fn restore_agent(
    state: &AppState,
    row: &Value,
    requested: Option<Uuid>,
) -> Result<Uuid, ExportError> {
    let recorded = row["assigned_agent_id"]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok());
    let agent_id = requested.or(recorded).ok_or(ExportError::AgentRequired)?;
    Agents::find_by_id(agent_id)
        .one(&state.db_conn)
        .await?
        .ok_or_else(|| ExportError::ResourceNotFound(format!("agent {agent_id}")))?;
    Ok(agent_id)
}

pub async fn restore(
    state: &AppState,
    reader: &Reader,
    workload_id: Uuid,
    requested_agent: Option<Uuid>,
) -> Result<Value, ExportError> {
    let row: Value = serde_json::from_slice(&reader.bytes(&row_name(workload_id)).await?)?;
    let image = reader.stream(&disk_name(workload_id)).await?;
    let agent_id = restore_agent(state, &row, requested_agent).await?;
    agent_request(
        state,
        agent_id,
        workload_id,
        Method::PUT,
        Some(request_body(image)),
    )
    .await?;
    Ok(json!({ "workload_id": workload_id, "agent_id": agent_id }))
}
