use std::net::SocketAddr;

use failover_controller::{log_error, log_info, logger, server, services};

const LEADER_KEY: &str = "/csfx/leaders/failover-controller";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    dotenvy::dotenv().ok();

    let log_receiver = logger::init_logger();

    log_info!("main", "CSFX Failover Controller starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    log_info!("main", "Connecting to database...");
    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");
    shared::spawn_log_writer(log_receiver, db.clone());

    let state = server::AppState { db: db.clone() };

    let app = server::create_router(state);

    shared::observability::spawn_pool_gauges("default", db.clone());
    shared::observability::serve_metrics("127.0.0.1:9004").await;

    let port = std::env::var("FAILOVER_CONTROLLER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8004);

    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: SocketAddr = format!("{}:{}", listen_addr, port).parse().unwrap();
    log_info!(
        "main",
        &format!("Failover Controller listening port={}", port)
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let leader = shared::leader::LeaderLease::connect(LEADER_KEY).await?;

    let retention = services::retention::Policy::from_env().unwrap_or_else(|e| {
        log_error!(
            "main",
            &format!("Invalid retention configuration err={}", e)
        );
        std::process::exit(1);
    });

    log_info!("main", "Starting retention loop");
    tokio::spawn(services::retention::run(
        db.clone(),
        leader.clone(),
        retention,
    ));

    log_info!("main", "Starting health monitor loop");
    tokio::spawn(services::monitor::run(db, leader));

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

    log_info!("main", "Failover Controller shutting down");
    Ok(())
}
