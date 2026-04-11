use std::net::SocketAddr;
use std::sync::Arc;

mod ceph;
mod db;
mod etcd;
mod handlers;
mod logger;
mod metrics;
mod models;
mod patroni;
mod server;
mod services;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    logger::init_logger();

    metrics::init();
    log_info!("main", "CSFX Volume Manager starting");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

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

    let etcd = Arc::new(tokio::sync::Mutex::new(etcd));

    let ceph_manager = match ceph::ops::init_ceph().await {
        Ok(manager) => {
            log_info!("main", "Ceph storage initialized");
            Some(Arc::new(manager))
        }
        Err(e) => {
            log_warn!(
                "main",
                &format!("Ceph not available (continuing without): {}", e)
            );
            None
        }
    };

    let volume_service = Arc::new(services::volume::VolumeService::new(
        db.clone(),
        etcd.clone(),
        ceph_manager,
    ));

    let state = server::AppState {
        db,
        etcd,
        volume_service,
    };

    let app = server::create_router(state);

    let port = std::env::var("VOLUME_MANAGER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8003);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
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
