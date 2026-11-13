use migration::run_pending_migrations;
use std::env;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("csfx_migrate=info".parse().unwrap()),
        )
        .init();

    let database_url = env::var("DATABASE_DIRECT_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .expect("DATABASE_URL must be set");

    tracing::info!("running pending migrations");

    if let Err(e) = run_pending_migrations(&database_url).await {
        tracing::error!(error = %e, "migration failed");
        std::process::exit(1);
    }

    tracing::info!("migrations complete");
}
