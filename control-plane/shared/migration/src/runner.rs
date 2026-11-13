use crate::concurrent_indexes::{build_concurrent_indexes, CONCURRENT_INDEXES};
use crate::Migrator;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use std::time::Duration;

const ADVISORY_LOCK_KEY: i64 = 0x6373_6678_6d69_6772;
const LOCK_CONNECTION_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

async fn connect_lock_session(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(LOCK_CONNECTION_LIFETIME)
        .max_lifetime(LOCK_CONNECTION_LIFETIME)
        .sqlx_logging(false);
    Database::connect(options).await
}

async fn apply_one_at_a_time(db: &DatabaseConnection) -> Result<(), DbErr> {
    while !Migrator::get_pending_migrations(db).await?.is_empty() {
        Migrator::up(db, Some(1)).await?;
    }
    Ok(())
}

pub async fn run_pending_migrations(database_url: &str) -> Result<(), DbErr> {
    let lock_session = connect_lock_session(database_url).await?;
    lock_session
        .execute_unprepared(&format!("SELECT pg_advisory_lock({ADVISORY_LOCK_KEY})"))
        .await?;
    let db = Database::connect(database_url).await?;
    let applied = async {
        apply_one_at_a_time(&db).await?;
        build_concurrent_indexes(&db, CONCURRENT_INDEXES).await
    }
    .await;
    let released = lock_session
        .execute_unprepared(&format!("SELECT pg_advisory_unlock({ADVISORY_LOCK_KEY})"))
        .await;
    applied?;
    released.map(|_| ())
}
