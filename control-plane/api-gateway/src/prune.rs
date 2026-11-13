use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, DbConn, DbErr, Statement};
use shared::env::parsed;
use tokio::time::{interval, Duration};

use crate::routes::settings::load_retention_days;
use crate::AppState;

const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);
const BATCH_ROWS: u64 = 10_000;
const AUDIT_RETENTION_ENV: &str = "CSFX_AUDIT_RETENTION_DAYS";
const DEFAULT_AUDIT_RETENTION_DAYS: i64 = 365;

const PRUNE_LOGS: &str = "
DELETE FROM logs WHERE ctid IN (
    SELECT ctid FROM logs WHERE last_seen_at < $1 LIMIT 10000
)";

const PRUNE_AUDIT: &str = "
DELETE FROM audit_log WHERE ctid IN (
    SELECT ctid FROM audit_log WHERE last_seen_at < $1 LIMIT 10000
)";

pub fn spawn_log_prune_job(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(PRUNE_INTERVAL);
        loop {
            ticker.tick().await;
            prune_once(&state).await;
        }
    });
}

fn audit_retention_days() -> i64 {
    parsed(AUDIT_RETENTION_ENV, DEFAULT_AUDIT_RETENTION_DAYS)
        .ok()
        .filter(|days| *days > 0)
        .unwrap_or(DEFAULT_AUDIT_RETENTION_DAYS)
}

async fn prune_once(state: &AppState) {
    match load_retention_days(state).await {
        Ok(days) => report("logs", days, prune_logs(&state.db_conn, cutoff(days)).await),
        Err(_) => tracing::warn!("skipping log prune, retention setting unavailable"),
    }
    let days = audit_retention_days();
    report(
        "audit",
        days,
        prune_audit(&state.db_conn, cutoff(days)).await,
    );
}

fn cutoff(retention_days: i64) -> DateTime<Utc> {
    Utc::now() - chrono::Duration::days(retention_days)
}

fn report(table: &str, retention_days: i64, result: Result<u64, DbErr>) {
    match result {
        Ok(rows_deleted) => tracing::info!(
            table = table,
            rows_deleted = rows_deleted,
            retention_days = retention_days,
            "pruned expired rows"
        ),
        Err(error) => {
            tracing::error!(table = table, error = %error, "failed to prune expired rows")
        }
    }
}

pub async fn prune_logs(db: &DbConn, cutoff: DateTime<Utc>) -> Result<u64, DbErr> {
    delete_batched(db, PRUNE_LOGS, cutoff).await
}

pub async fn prune_audit(db: &DbConn, cutoff: DateTime<Utc>) -> Result<u64, DbErr> {
    delete_batched(db, PRUNE_AUDIT, cutoff).await
}

async fn delete_batched(db: &DbConn, sql: &str, cutoff: DateTime<Utc>) -> Result<u64, DbErr> {
    let mut total = 0;
    loop {
        let statement = Statement::from_sql_and_values(DbBackend::Postgres, sql, [cutoff.into()]);
        let deleted = db.execute(statement).await?.rows_affected();
        total += deleted;
        if deleted < BATCH_ROWS {
            return Ok(total);
        }
    }
}
