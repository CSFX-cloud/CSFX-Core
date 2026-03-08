use std::net::SocketAddr;
use std::sync::Arc;

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
    log_info!("main", "CSF Registry Service starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    log_info!("main", "Connecting to database...");
    let db_conn = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");

    let cert_ttl_hours: i64 = std::env::var("CSF_CERT_TTL_HOURS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);

    let pki_service = services::pki::PkiService::new(db_conn.clone(), cert_ttl_hours)
        .expect("Failed to initialize PKI service");

    let token_manager = Arc::new(services::tokens::TokenManager::new(db_conn.clone()));
    let bootstrap_token_manager = Arc::new(services::bootstrap_tokens::BootstrapTokenManager::new(db_conn.clone()));
    let api_key_manager = Arc::new(services::api_keys::ApiKeyManager::new(db_conn.clone()));
    let agent_registry = Arc::new(services::registry::AgentRegistry::new(db_conn.clone()));

    log_info!("main", "Managers initialized");

    let scheduler_url = std::env::var("SCHEDULER_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:8002".to_string());

    let gateway_url = std::env::var("API_GATEWAY_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("Failed to build HTTP client");

    let state = server::AppState {
        token_manager: token_manager.clone(),
        bootstrap_token_manager: bootstrap_token_manager.clone(),
        api_key_manager: api_key_manager.clone(),
        agent_registry: agent_registry.clone(),
        pki_service: Arc::new(pki_service),
        db: db_conn.clone(),
        scheduler_url,
        gateway_url,
        http_client,
    };

    let token_cleanup_handle = {
        let token_mgr = token_manager.clone();
        let bootstrap_mgr = bootstrap_token_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                token_mgr.cleanup_expired().await;
                bootstrap_mgr.cleanup_expired().await;
            }
        })
    };

    let health_check_handle = {
        let registry = agent_registry.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                registry.check_agent_health(300).await;
            }
        })
    };

    let app = server::create_router(state);

    let port = std::env::var("REGISTRY_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8001);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    log_info!("main", &format!("Registry listening port={}", port));

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::select! {
        _ = server_handle => {
            log_error!("main", "Server stopped unexpectedly");
        }
        _ = token_cleanup_handle => {
            log_error!("main", "Token cleanup task stopped unexpectedly");
        }
        _ = health_check_handle => {
            log_error!("main", "Health check task stopped unexpectedly");
        }
        _ = tokio::signal::ctrl_c() => {
            log_info!("main", "Shutdown signal received");
        }
    }

    log_info!("main", "Registry Service shutting down");
    Ok(())
}
