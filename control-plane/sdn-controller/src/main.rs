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
    log_info!("main", "CSF SDN Controller starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    log_info!("main", "Connecting to database...");
    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");

    let etcd_url = std::env::var("ETCD_URL").unwrap_or_else(|_| "http://localhost:2379".to_string());
    log_info!("main", &format!("Connecting to etcd url={}", etcd_url));
    let etcd = etcd_client::Client::connect([etcd_url.as_str()], None)
        .await
        .expect("Failed to connect to etcd");
    log_info!("main", "etcd connection established");

    let ipam = services::ipam::IpamService::new(etcd);
    let state = server::AppState::new(db, ipam);
    let app = server::create_router(state);

    let port = std::env::var("SDN_CONTROLLER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8005);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log_info!("main", &format!("SDN Controller listening port={}", port));

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

    log_info!("main", "SDN Controller shutting down");
    Ok(())
}
