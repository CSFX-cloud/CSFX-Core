use migration::{Migrator, MigratorTrait};
use sea_orm::Database;
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

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let db = match Database::connect(&database_url).await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!(error = %e, "failed to connect to database");
            std::process::exit(1);
        }
    };

    tracing::info!("running pending migrations");

    if let Err(e) = Migrator::up(&db, None).await {
        tracing::error!(error = %e, "migration failed");
        std::process::exit(1);
    }

    tracing::info!("migrations complete");
}
