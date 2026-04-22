use std::net::SocketAddr;

mod db;
mod handlers;
mod logger;
mod metrics;
mod models;
mod server;
mod services;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    logger::init_logger();

    metrics::init();
    log_info!("main", "CSFX Failover Controller starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    log_info!("main", "Connecting to database...");
    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");

    let state = server::AppState { db: db.clone() };

    let app = server::create_router(state);

    let port = std::env::var("FAILOVER_CONTROLLER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8004);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log_info!(
        "main",
        &format!("Failover Controller listening port={}", port)
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;

    log_info!("main", "Starting health monitor loop");
    tokio::spawn(services::monitor::run(db));

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
