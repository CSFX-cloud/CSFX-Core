use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use site_builder::usage::{prune_old, UsageCounter};
use uuid::Uuid;

use crate::common::{insert_site, migrated_database};

async fn totals(db: &DatabaseConnection, site_id: Uuid) -> (i64, i64) {
    let statement = Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT COALESCE(SUM(requests), 0)::bigint, COALESCE(SUM(bytes_out), 0)::bigint FROM site_usage_hourly WHERE site_id = $1",
        [site_id.into()],
    );
    let row = db.query_one(statement).await.unwrap().unwrap();
    (
        row.try_get_by_index(0).unwrap(),
        row.try_get_by_index(1).unwrap(),
    )
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn flush_accumulates_across_calls() {
    let db = migrated_database().await;
    let site = insert_site(&db, "static", None).await;
    let counter = UsageCounter::default();
    counter.record(site.id, 100, Utc::now());
    counter.record(site.id, 50, Utc::now());
    counter.flush(&db).await.unwrap();
    counter.record(site.id, 25, Utc::now());
    counter.flush(&db).await.unwrap();
    assert!(counter.drain().is_empty());
    assert_eq!(totals(&db, site.id).await, (3, 175));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn prune_old_removes_rows_past_retention_only() {
    let db = migrated_database().await;
    let site = insert_site(&db, "static", None).await;
    let counter = UsageCounter::default();
    counter.record(site.id, 1, Utc::now() - Duration::days(120));
    counter.record(site.id, 2, Utc::now());
    counter.flush(&db).await.unwrap();
    assert_eq!(totals(&db, site.id).await, (2, 3));
    prune_old(&db).await.unwrap();
    assert_eq!(totals(&db, site.id).await, (1, 2));
}
