mod artifacts;
mod builds;
mod config;
mod edge;
mod error;
mod git;
mod job;
mod prune;
mod registry;
mod routes;
mod scheduler;
mod server;
mod sites;
mod token;

use anyhow::Context;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

const REAP_INTERVAL: Duration = Duration::from_secs(60);

fn spawn_reaper(builds: Arc<builds::Builds>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(REAP_INTERVAL);
        loop {
            tick.tick().await;
            if let Err(error) = builds.reap_stale().await {
                tracing::error!(error = %error, "stale build cleanup failed");
            }
        }
    });
}

async fn listen(router: axum::Router, port: u16, name: &'static str) -> anyhow::Result<()> {
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind {name} on {address}"))?;
    tracing::info!(listener = name, address = %address, "listening");
    axum::serve(listener, router).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install crypto provider"))?;
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Arc::new(config::Config::from_environment()?);
    let db = shared::establish_connection().await?;
    let scheduler = scheduler::SchedulerClient::new(&config.scheduler_url)?;
    let registry_keys = Arc::new(registry::keys::load_or_create(&db).await?);
    let registry_client = match config.registry.as_deref() {
        Some(registry) => Some(registry::client::RegistryClient::new(
            registry,
            config.registry_insecure,
            registry_keys.clone(),
        )?),
        None => None,
    };
    let builds = Arc::new(builds::Builds::new(
        db.clone(),
        config.clone(),
        scheduler,
        registry_client,
    ));
    spawn_reaper(builds.clone());

    let registry_state = registry::RegistryState {
        db: db.clone(),
        config: config.clone(),
        keys: registry_keys,
    };
    let control_app = server::control_router(builds).merge(registry::router(registry_state));
    let control = listen(control_app, config.port, "control");
    let edge = listen(
        server::edge_router(Arc::new(edge::Edge::new(db))),
        config.edge_port,
        "edge",
    );
    tokio::try_join!(control, edge)?;
    Ok(())
}
