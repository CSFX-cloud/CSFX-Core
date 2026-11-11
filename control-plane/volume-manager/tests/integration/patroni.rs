use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use volume_manager::patroni::client::PatroniClient;
use volume_manager::patroni::monitor::PatroniMonitor;
use volume_manager::patroni::types::{PatroniState, PostgresNodeRole};

type Failovers = Arc<Mutex<Vec<Value>>>;

#[derive(Clone)]
struct Mock {
    role: &'static str,
    failovers: Failovers,
}

async fn health(State(mock): State<Mock>) -> (StatusCode, String) {
    if mock.role == "garbage" {
        return (StatusCode::OK, "not json".to_string());
    }
    let body = json!({ "state": "running", "role": mock.role, "timeline": 3 });
    (StatusCode::OK, body.to_string())
}

async fn primary(State(mock): State<Mock>) -> StatusCode {
    match mock.role {
        "master" => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn replica(State(mock): State<Mock>) -> StatusCode {
    match mock.role {
        "replica" => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn failover(State(mock): State<Mock>, Json(body): Json<Value>) -> StatusCode {
    mock.failovers.lock().unwrap().push(body);
    StatusCode::OK
}

async fn spawn(role: &'static str, failovers: &Failovers) -> String {
    let mock = Mock {
        role,
        failovers: failovers.clone(),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/primary", get(primary))
        .route("/replica", get(replica))
        .route("/failover", post(failover))
        .with_state(mock);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

fn client(nodes: Vec<String>) -> PatroniClient {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    PatroniClient::new("scope".to_string(), nodes)
}

#[tokio::test]
async fn cluster_status_reports_roles_and_unreachable_nodes() {
    let failovers = Failovers::default();
    let nodes = vec![
        spawn("master", &failovers).await,
        spawn("replica", &failovers).await,
        spawn("garbage", &failovers).await,
        "http://127.0.0.1:1".to_string(),
    ];
    let cluster = client(nodes.clone()).get_cluster_status().await.unwrap();
    assert_eq!(cluster.scope, "scope");
    assert_eq!(cluster.leader.as_deref(), Some("127.0.0.1"));
    assert_eq!(cluster.members.len(), 4);
    assert_eq!(cluster.members[0].role, PostgresNodeRole::Primary);
    assert_eq!(cluster.members[0].state, PatroniState::Running);
    assert_eq!(cluster.members[0].timeline, Some(3));
    assert_eq!(cluster.members[1].role, PostgresNodeRole::Replica);
    assert_eq!(cluster.members[2].state, PatroniState::Failed);
    assert_eq!(cluster.members[3].role, PostgresNodeRole::Unknown);
    assert_eq!(
        cluster.members[3].postgres_url,
        "postgresql://127.0.0.1:5432"
    );
    assert_eq!(cluster.members[1].api_url, nodes[1]);
}

#[tokio::test]
async fn primary_and_replica_probes() {
    let failovers = Failovers::default();
    let primary = spawn("master", &failovers).await;
    let replica = spawn("replica", &failovers).await;
    let client = client(vec![primary.clone(), replica.clone()]);
    assert!(client.is_primary(&primary).await.unwrap());
    assert!(!client.is_primary(&replica).await.unwrap());
    assert!(client.is_replica(&replica).await.unwrap());
    assert!(!client.is_replica(&primary).await.unwrap());
    assert!(client.is_primary("http://127.0.0.1:1").await.is_err());
}

#[tokio::test]
async fn find_primary_and_replicas() {
    let failovers = Failovers::default();
    let nodes = vec![
        spawn("master", &failovers).await,
        spawn("replica", &failovers).await,
        spawn("replica", &failovers).await,
    ];
    let client = client(nodes);
    assert!(client.find_primary().await.unwrap().is_some());
    assert_eq!(client.find_replicas().await.unwrap().len(), 2);
}

#[tokio::test]
async fn failover_is_posted_to_the_primary() {
    let failovers = Failovers::default();
    let nodes = vec![
        spawn("replica", &failovers).await,
        spawn("master", &failovers).await,
    ];
    let client = client(nodes);
    client.trigger_failover(None).await.unwrap();
    client.trigger_failover(Some("replica-1")).await.unwrap();
    let recorded = failovers.lock().unwrap().clone();
    assert_eq!(recorded[0], json!({ "leader": "127.0.0.1" }));
    assert_eq!(recorded[1]["candidate"], "replica-1");
}

#[tokio::test]
async fn failover_without_primary_fails() {
    let failovers = Failovers::default();
    let client = client(vec![spawn("replica", &failovers).await]);
    assert!(client.find_primary().await.unwrap().is_none());
    let error = client.trigger_failover(None).await.unwrap_err();
    assert!(error.to_string().contains("No primary found"));
}

#[tokio::test]
async fn monitor_reports_cluster_readiness() {
    let failovers = Failovers::default();
    let nodes = vec![
        spawn("master", &failovers).await,
        spawn("replica", &failovers).await,
    ];
    let monitor = PatroniMonitor::new(client(nodes.clone()), 1);
    monitor.wait_for_cluster_ready(5).await.unwrap();
    assert!(monitor.is_cluster_healthy().await);
    assert!(monitor.get_primary().await.unwrap().is_some());
    assert_eq!(monitor.get_replicas().await.unwrap().len(), 1);

    let mut degraded = nodes;
    degraded.push("http://127.0.0.1:1".to_string());
    let degraded = PatroniMonitor::new(client(degraded), 1);
    assert!(!degraded.is_cluster_healthy().await);
}

#[tokio::test]
async fn monitor_times_out_without_primary() {
    let failovers = Failovers::default();
    let nodes = vec![spawn("replica", &failovers).await];
    let monitor = PatroniMonitor::new(client(nodes), 1);
    assert!(!monitor.is_cluster_healthy().await);
    let error = monitor.wait_for_cluster_ready(0).await.unwrap_err();
    assert!(error.to_string().contains("Timeout"));
}

#[tokio::test]
async fn monitoring_loop_survives_unhealthy_clusters() {
    let failovers = Failovers::default();
    let headless = vec![
        spawn("replica", &failovers).await,
        "http://127.0.0.1:1".to_string(),
    ];
    let monitor = Arc::new(PatroniMonitor::new(client(headless), 1));
    let handle = tokio::spawn(monitor.start_monitoring());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!handle.is_finished());
    handle.abort();
}
