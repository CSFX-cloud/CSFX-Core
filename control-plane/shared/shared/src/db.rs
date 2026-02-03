use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DbConn, DbErr};
use std::env;

/// Establish a database connection and run migrations
/// 
/// This function:
/// 1. Reads DATABASE_URL from environment
/// 2. Connects to the PostgreSQL database
/// 3. Runs all pending migrations
/// 
/// # Errors
/// Returns DbErr if connection or migration fails
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_url_required() {
        // This would panic if DATABASE_URL is not set
        // In real tests, you'd mock the environment
    }
}
