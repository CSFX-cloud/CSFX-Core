use std::net::SocketAddr;
use std::sync::Arc;

mod api_keys;
mod logger;
mod registry;
mod server;
mod tokens;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    // Initialize logger
    logger::init_logger();

    log_info!("main", "CSF Registry Service starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    // Initialize managers
    let token_manager = Arc::new(tokens::TokenManager::new());
    let api_key_manager = Arc::new(api_keys::ApiKeyManager::new());
    let agent_registry = Arc::new(registry::AgentRegistry::new());

    log_info!("main", "Managers initialized");

    // Create application state
    let state = server::AppState {
        token_manager: token_manager.clone(),
        api_key_manager: api_key_manager.clone(),
        agent_registry: agent_registry.clone(),
    };

    // Start background tasks
    // 1. Token cleanup task
    let token_cleanup_handle = {
        let token_mgr = token_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Every hour
            loop {
                interval.tick().await;
                token_mgr.cleanup_expired().await;
            }
        })
    };

    // 2. Agent health check task
    let health_check_handle = {
        let registry = agent_registry.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Every minute
            loop {
                interval.tick().await;
                registry.check_agent_health(300).await; // 5 minutes timeout
            }
        })
    };

    // Create router
    let app = server::create_router(state);

    // Get port from environment or use default
    let port = std::env::var("REGISTRY_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8001);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    log_info!("main", &format!("Registry listening on {}", addr));
    log_info!("main", "Endpoints:");
    log_info!("main", "   - GET  /health");
    log_info!("main", "   - POST /admin/tokens");
    log_info!("main", "   - GET  /admin/tokens");
    log_info!("main", "   - POST /agents/register");
    log_info!("main", "   - POST /agents/:id/heartbeat");
    log_info!("main", "   - GET  /admin/agents");
    log_info!("main", "   - GET  /admin/statistics");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Serve the application
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for all tasks (or until interrupted)
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
