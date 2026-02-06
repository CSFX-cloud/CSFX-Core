use super::{EtcdClient, EtcdConfig, HealthChecker, LeaderElection, StateManager};
use crate::log_info;
use crate::logger;
use std::sync::Arc;
use uuid;

pub struct InitData {
    pub etcd_client: Arc<EtcdClient>,
    pub state_manager: Arc<StateManager>,
    pub health_checker: Arc<HealthChecker>,
    pub leader_election: Arc<LeaderElection>,
    pub node_id: String,
}

pub async fn init_cluster() -> anyhow::Result<InitData> {
    logger::init_logger();

    let etcd_config = EtcdConfig::from_env();
    let etcd_client = Arc::new(EtcdClient::new(etcd_config));

    etcd_client.connect().await?;
    log_info!("etcd::init", "Connected to etcd cluster");

    let state_manager = Arc::new(StateManager::new(etcd_client.clone()));
    let health_checker = Arc::new(HealthChecker::new(etcd_client.clone()));

    let node_id = std::env::var("NODE_ID")
        .unwrap_or_else(|_| format!("volume-manager-{}", uuid::Uuid::new_v4()));
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let ip_address = std::env::var("NODE_IP").unwrap_or_else(|_| "127.0.0.1".to_string());

    log_info!(
        "etcd::init",
        &format!("Registering node with ID: {}", node_id)
    );
    state_manager
        .register_node(node_id.clone(), hostname, ip_address)
        .await?;

    // Leader Election starten
    let leader_election = Arc::new(LeaderElection::new(etcd_client.clone(), node_id.clone()));
    leader_election.campaign().await?;

    Ok(InitData {
        etcd_client,
        state_manager,
        health_checker,
        leader_election,
        node_id,
    })
}
