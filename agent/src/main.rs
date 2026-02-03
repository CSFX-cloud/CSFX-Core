use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    // Initialize shared logger
    shared::init_logger();

    tracing::info!("🤖 CSF Agent starting...");
    tracing::info!("Version: {}", env!("CARGO_PKG_VERSION"));
    
    // TODO: Implement agent functionality
    // Agent will:
    // - Monitor system resources
    // - Report metrics to control plane
    // - Execute tasks from control plane
    // - Manage local Docker containers
    
    tracing::warn!("⚠️  Agent functionality not yet implemented");
    tracing::info!("✅ Test log: Agent initialized successfully");
    
    // Keep the agent running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        tracing::debug!("Agent heartbeat...");
    }
}
