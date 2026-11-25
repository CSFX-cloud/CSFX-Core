use std::net::SocketAddr;
use std::sync::Arc;

use shared::leader::LeaderLease;
use volume_manager::{ceph, log_error, log_info, logger, server, services};

const LEADER_KEY: &str = "/csfx/leaders/volume-manager";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    dotenvy::dotenv().ok();
    shared::internal_auth::ensure_configured().map_err(anyhow::Error::msg)?;

    let log_receiver = logger::init_logger();

    log_info!("main", "CSFX Volume Manager starting");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");
    shared::spawn_log_writer(log_receiver, db.clone());
    shared::observability::spawn_pool_gauges("default", db.clone());

    let ceph_manager = if std::env::var("CEPH_MON_HOSTS").is_ok() {
        let manager = ceph::ops::init_ceph().await?;
        log_info!("main", "Ceph storage initialized");
        Some(Arc::new(manager))
    } else {
        log_info!("main", "CEPH_MON_HOSTS not set, running without Ceph");
        None
    };

    let volume_service = Arc::new(services::volume::VolumeService::new(
        db.clone(),
        ceph_manager.clone(),
    ));

    if let Some(ceph) = ceph_manager {
        let leader = LeaderLease::connect(LEADER_KEY).await?;
        tokio::spawn(services::reconcile::run(
            db.clone(),
            ceph,
            volume_service.clone(),
            leader,
        ));
    }

    let state = server::AppState { db, volume_service };

    let app = server::create_router(state).layer(axum::middleware::from_fn(
        shared::internal_auth::require_service_token,
    ));

    shared::observability::serve_metrics("127.0.0.1:9003").await;

    let port = std::env::var("VOLUME_MANAGER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8003);

    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: SocketAddr = format!("{}:{}", listen_addr, port).parse().unwrap();
    log_info!("main", &format!("Volume Manager listening port={}", port));

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                log_error!("main", &format!("Server error err={}", e));
            }
        }
        _ = tokio::signal::ctrl_c() => {
            log_info!("main", "Shutdown signal received");
        }
    }

    log_info!("main", "Volume Manager shutting down");
    Ok(())
}
