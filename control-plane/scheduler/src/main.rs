use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    log_info!("main", "CSFX Scheduler Service starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    log_info!("main", "Connecting to database...");
    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");

    let etcd_endpoints =
        std::env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| "http://localhost:2379".to_string());
    let etcd_endpoints: Vec<&str> = etcd_endpoints.split(',').collect();

    log_info!(
        "main",
        &format!("Connecting to etcd endpoints={}", etcd_endpoints.join(","))
    );
    let etcd = etcd_client::Client::connect(etcd_endpoints, None)
        .await
        .expect("Failed to connect to etcd");
    log_info!("main", "etcd connection established");

    let etcd = Arc::new(Mutex::new(etcd));
    let scheduler = Arc::new(services::scheduler::SchedulerService::new(
        db.clone(),
        etcd.clone(),
    ));

    let state = server::AppState {
        db,
        etcd,
        scheduler,
    };

    let app = server::create_router(state);

    let port = std::env::var("SCHEDULER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8002);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log_info!("main", &format!("Scheduler listening port={}", port));

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

    log_info!("main", "Scheduler Service shutting down");
    Ok(())
}
