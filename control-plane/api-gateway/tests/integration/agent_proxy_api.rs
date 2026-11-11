use axum::http::{Method, StatusCode};
use entity::entities::agents;
use futures_util::{SinkExt, StreamExt};
use sea_orm::ActiveValue::Set;
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::common::{harness, Harness};
use crate::support_a::{
    insert_agent, insert_group, insert_workload, plain_harness, prefixed_id, raw, scripted_harness,
    serve, update_agent, WorkloadSpec, FAILING_PREFIX,
};

async fn hosted_workload(harness: &Harness, id: Option<Uuid>) -> (Uuid, agents::Model) {
    let group = insert_group(&harness.db, harness.org_id).await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let mut spec = WorkloadSpec::new(group.id, Some(agent.id), "running");
    spec.id = id.unwrap_or(spec.id);
    let workload = insert_workload(&harness.db, spec).await;
    (workload.id, agent)
}

async fn ticket(harness: &Harness, uri: &str) -> String {
    let (status, body) = harness.admin(Method::POST, uri, None).await;
    assert_eq!(status, StatusCode::OK);
    body["ticket"].as_str().unwrap().to_string()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn proxy_routes_require_permissions() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    let id = Uuid::new_v4();
    let cases = [
        (Method::GET, format!("/api/workloads/{id}/logs")),
        (Method::POST, format!("/api/workloads/{id}/exec/ticket")),
        (Method::POST, format!("/api/workloads/{id}/vnc/ticket")),
        (Method::POST, format!("/api/agents/{id}/metrics/ticket")),
        (Method::POST, format!("/api/agents/{id}/power")),
        (Method::GET, format!("/api/agents/{id}/disks")),
    ];
    for (method, uri) in cases {
        let body = (method == Method::POST).then(|| json!({"action": "reboot"}));
        let (status, _) = harness
            .send(method.clone(), &uri, Some(&limited), body.clone())
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        let (status, _) = harness.send(method, &uri, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn tickets_are_issued_to_authorized_users() {
    let harness = harness().await;
    let id = Uuid::new_v4();
    for uri in [
        format!("/api/workloads/{id}/exec/ticket"),
        format!("/api/workloads/{id}/vnc/ticket"),
        format!("/api/agents/{id}/metrics/ticket"),
    ] {
        assert!(!ticket(&harness, &uri).await.is_empty());
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn workload_logs_report_missing_targets() {
    let harness = scripted_harness().await;
    let unknown = format!("/api/workloads/{}/logs", Uuid::new_v4());
    assert_eq!(harness.get(&unknown).await.0, StatusCode::NOT_FOUND);

    let group = insert_group(&harness.db, harness.org_id).await;
    let unassigned =
        insert_workload(&harness.db, WorkloadSpec::new(group.id, None, "pending")).await;
    let (status, _) = harness
        .get(&format!("/api/workloads/{}/logs", unassigned.id))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (workload_id, agent) = hosted_workload(&harness, None).await;
    update_agent(&harness.db, agent, |active| active.wg_tunnel_ip = Set(None)).await;
    let (status, _) = harness
        .get(&format!("/api/workloads/{workload_id}/logs"))
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn workload_logs_stream_from_the_agent() {
    let harness = scripted_harness().await;
    let (workload_id, _) = hosted_workload(&harness, None).await;
    let (status, _, body) = raw(
        &harness,
        Method::GET,
        &format!("/api/workloads/{workload_id}/logs"),
        &[("authorization", &format!("Bearer {}", harness.token))],
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), b"first line\nsecond line\n");

    let (refused, _) = hosted_workload(&harness, Some(prefixed_id(FAILING_PREFIX))).await;
    let (status, body) = harness.get(&format!("/api/workloads/{refused}/logs")).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "agent refused log stream request");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registry_without_ticket_fields_yields_bad_gateway() {
    let harness = plain_harness().await;
    let (workload_id, agent) = hosted_workload(&harness, None).await;
    let agent_id = agent.id;
    let (status, body) = harness
        .get(&format!("/api/workloads/{workload_id}/logs"))
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "malformed proxy ticket payload");
    let (status, _) = harness
        .admin(
            Method::POST,
            &format!("/api/agents/{agent_id}/power"),
            Some(json!({"action": "reboot"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn power_actions_reach_the_agent() {
    let harness = scripted_harness().await;
    let (_, agent) = hosted_workload(&harness, None).await;
    let agent_id = agent.id;
    let uri = format!("/api/agents/{agent_id}/power");

    let (status, _) = harness
        .admin(Method::POST, &uri, Some(json!({"action": "reboot"})))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = harness
        .admin(Method::POST, &uri, Some(json!({"action": "poweroff"})))
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "agent refused power action");
    let (status, _) = harness
        .admin(Method::POST, &uri, Some(json!({"action": "explode"})))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let missing = format!("/api/agents/{}/power", Uuid::new_v4());
    let (status, _) = harness
        .admin(Method::POST, &missing, Some(json!({"action": "reboot"})))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_disks_are_proxied() {
    let harness = scripted_harness().await;
    let (_, agent) = hosted_workload(&harness, None).await;
    let agent_id = agent.id;
    let (status, disks) = harness.get(&format!("/api/agents/{agent_id}/disks")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(disks[0]["device"], "sda");
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    update_agent(&harness.db, agent.clone(), |active| {
        active.wg_tunnel_ip = Set(None)
    })
    .await;
    let (status, _) = harness
        .get(&format!("/api/agents/{}/disks", agent.id))
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, _) = harness
        .get(&format!("/api/agents/{}/disks", Uuid::new_v4()))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn connect(
    address: std::net::SocketAddr,
    path: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::Error,
> {
    let url = format!("ws://{address}/api{path}");
    tokio_tungstenite::connect_async(url)
        .await
        .map(|pair| pair.0)
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn exec_and_vnc_sockets_bridge_to_the_agent() {
    let harness = scripted_harness().await;
    let address = serve(&harness).await;
    let (workload_id, _) = hosted_workload(&harness, None).await;

    for kind in ["exec", "vnc"] {
        let ticket = ticket(
            &harness,
            &format!("/api/workloads/{workload_id}/{kind}/ticket"),
        )
        .await;
        let mut socket = connect(
            address,
            &format!("/workloads/{workload_id}/{kind}?ticket={ticket}"),
        )
        .await
        .unwrap();
        socket.send(Message::Text("ping".into())).await.unwrap();
        let reply = socket.next().await.unwrap().unwrap();
        assert_eq!(reply, Message::Text("ping".into()), "{kind}");
        let rejected = connect(
            address,
            &format!("/workloads/{workload_id}/{kind}?ticket=bad"),
        )
        .await;
        assert!(rejected.is_err(), "{kind}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn exec_socket_fails_for_unresolvable_targets() {
    let harness = scripted_harness().await;
    let address = serve(&harness).await;
    let unknown = Uuid::new_v4();
    let ticket = ticket(&harness, &format!("/api/workloads/{unknown}/exec/ticket")).await;
    let result = connect(
        address,
        &format!("/workloads/{unknown}/exec?ticket={ticket}"),
    )
    .await;
    assert!(result.is_err());
    let other = Uuid::new_v4();
    let stale = ticket_for(&harness, other).await;
    assert!(connect(
        address,
        &format!("/workloads/{unknown}/exec?ticket={stale}")
    )
    .await
    .is_err());
}

async fn ticket_for(harness: &Harness, workload_id: Uuid) -> String {
    ticket(harness, &format!("/api/workloads/{workload_id}/vnc/ticket")).await
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn node_metrics_stream_forwards_agent_frames() {
    let harness = scripted_harness().await;
    let address = serve(&harness).await;
    let (_, agent) = hosted_workload(&harness, None).await;
    let agent_id = agent.id;
    let ticket = ticket(&harness, &format!("/api/agents/{agent_id}/metrics/ticket")).await;

    let mut socket = connect(
        address,
        &format!("/agents/{agent_id}/metrics/stream?ticket={ticket}"),
    )
    .await
    .unwrap();
    let frame = socket.next().await.unwrap().unwrap();
    assert_eq!(frame, Message::Text("{\"cpu\":1}".into()));
    let rejected = connect(
        address,
        &format!("/agents/{agent_id}/metrics/stream?ticket=bad"),
    )
    .await;
    assert!(rejected.is_err());
}
