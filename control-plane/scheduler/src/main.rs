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
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    dotenvy::dotenv().ok();

    let log_receiver = logger::init_logger();

    metrics::init();
    log_info!("main", "CSFX Scheduler Service starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    log_info!("main", "Connecting to database...");
    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");
    shared::spawn_log_writer(log_receiver, db.clone());

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

    let app = server::create_router(state.clone());

    let retry_scheduler = state.scheduler.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            match retry_scheduler.retry_pending().await {
                Ok(0) => {}
                Ok(count) => {
                    log_info!(
                        "main",
                        &format!("Retried pending workloads placed={}", count)
                    );
                }
                Err(e) => {
                    log_error!("main", &format!("Pending retry failed err={}", e));
                }
            }
        }
    });

    let port = std::env::var("SCHEDULER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8002);

    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: SocketAddr = format!("{}:{}", listen_addr, port).parse().unwrap();
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
