use crate::concurrent_indexes::{build_concurrent_indexes, CONCURRENT_INDEXES};
use crate::Migrator;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement,
};
use std::time::Duration;

const ADVISORY_LOCK_KEY: i64 = 0x6373_6678_6d69_6772;
const LOCK_CONNECTION_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(500);

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

async fn try_lock(lock_session: &DatabaseConnection) -> Result<bool, DbErr> {
    let row = lock_session
        .query_one(Statement::from_string(
            lock_session.get_database_backend(),
            format!("SELECT pg_try_advisory_lock({ADVISORY_LOCK_KEY}) AS locked"),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("advisory lock query returned no row".to_string()))?;
    row.try_get("", "locked")
}

async fn wait_for_lock(lock_session: &DatabaseConnection) -> Result<(), DbErr> {
    while !try_lock(lock_session).await? {
        tokio::time::sleep(LOCK_RETRY_INTERVAL).await;
    }
    Ok(())
}

async fn apply_one_at_a_time(db: &DatabaseConnection) -> Result<(), DbErr> {
    while !Migrator::get_pending_migrations(db).await?.is_empty() {
        Migrator::up(db, Some(1)).await?;
    }
    Ok(())
}

pub async fn run_pending_migrations(database_url: &str) -> Result<(), DbErr> {
    let lock_session = connect_lock_session(database_url).await?;
    wait_for_lock(&lock_session).await?;
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
