use chrono::{DateTime, Duration, NaiveDateTime, Timelike, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

const RETENTION_DAYS: i64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totals {
    pub requests: i64,
    pub bytes_out: i64,
}

type Key = (Uuid, NaiveDateTime);

#[derive(Default)]
pub struct UsageCounter {
    buckets: Mutex<HashMap<Key, Totals>>,
}

pub fn hour_of(moment: DateTime<Utc>) -> NaiveDateTime {
    moment
        .naive_utc()
        .with_minute(0)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or_else(|| moment.naive_utc())
}

fn add(target: &mut Totals, other: Totals) {
    target.requests += other.requests;
    target.bytes_out += other.bytes_out;
}

impl UsageCounter {
    pub fn record(&self, site_id: Uuid, bytes: usize, moment: DateTime<Utc>) {
        let bytes_out = i64::try_from(bytes).unwrap_or(i64::MAX);
        self.merge(
            (site_id, hour_of(moment)),
            Totals {
                requests: 1,
                bytes_out,
            },
        );
    }

    fn merge(&self, key: Key, totals: Totals) {
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        add(buckets.entry(key).or_default(), totals);
    }

    pub fn drain(&self) -> Vec<(Key, Totals)> {
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *buckets).into_iter().collect()
    }

    pub async fn flush(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        let rows = self.drain();
        let mut failure = None;
        for (key, totals) in rows {
            if let Err(error) = upsert(db, key, totals).await {
                self.merge(key, totals);
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

async fn upsert(db: &DatabaseConnection, key: Key, totals: Totals) -> Result<(), DbErr> {
    let statement = Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO site_usage_hourly (site_id, hour, requests, bytes_out) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (site_id, hour) DO UPDATE SET \
         requests = site_usage_hourly.requests + EXCLUDED.requests, \
         bytes_out = site_usage_hourly.bytes_out + EXCLUDED.bytes_out",
        [
            key.0.into(),
            key.1.into(),
            totals.requests.into(),
            totals.bytes_out.into(),
        ],
    );
    db.execute(statement).await?;
    Ok(())
}

pub async fn prune_old(db: &DatabaseConnection) -> Result<(), DbErr> {
    let cutoff = (Utc::now() - Duration::days(RETENTION_DAYS)).naive_utc();
    let statement = Statement::from_sql_and_values(
        db.get_database_backend(),
        "DELETE FROM site_usage_hourly WHERE hour < $1",
        [cutoff.into()],
    );
    db.execute(statement).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 1, hour, minute, 30).unwrap()
    }

    #[test]
    fn requests_in_the_same_hour_are_summed() {
        let counter = UsageCounter::default();
        let site = Uuid::from_u128(1);
        counter.record(site, 100, at(10, 5));
        counter.record(site, 50, at(10, 55));
        counter.record(site, 10, at(11, 0));
        let mut rows = counter.drain();
        rows.sort_by_key(|(key, _)| key.1);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].1,
            Totals {
                requests: 2,
                bytes_out: 150
            }
        );
        assert_eq!(
            rows[1].1,
            Totals {
                requests: 1,
                bytes_out: 10
            }
        );
    }

    #[test]
    fn drain_empties_the_counter() {
        let counter = UsageCounter::default();
        counter.record(Uuid::nil(), 1, at(1, 0));
        assert_eq!(counter.drain().len(), 1);
        assert!(counter.drain().is_empty());
    }

    #[test]
    fn hours_are_truncated_to_the_hour() {
        let truncated = hour_of(at(7, 42));
        assert_eq!((truncated.minute(), truncated.second()), (0, 0));
    }

    #[test]
    fn different_sites_are_counted_separately() {
        let counter = UsageCounter::default();
        counter.record(Uuid::from_u128(1), 1, at(1, 0));
        counter.record(Uuid::from_u128(2), 1, at(1, 0));
        assert_eq!(counter.drain().len(), 2);
    }
}
