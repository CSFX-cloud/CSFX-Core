use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DbConn, DbErr};
use std::env;

pub async fn establish_connection() -> Result<DbConn, DbErr> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    tracing::info!("Connecting to PostgreSQL database...");
    let db_conn = Database::connect(&database_url).await?;

    // Run pending migrations
    tracing::info!("Running database migrations...");
    Migrator::up(&db_conn, None).await?;
    tracing::info!("Database migrations completed successfully");

    Ok(db_conn)
}
