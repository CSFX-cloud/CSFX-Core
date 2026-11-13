use chrono::{Duration as Span, NaiveDateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement};
use shared::env::parsed;
use shared::leader::LeaderLease;
use tokio::time::{interval, Duration};

const RUN_INTERVAL: Duration = Duration::from_secs(3600);
const BATCH_ROWS: u64 = 10_000;

const THIN_METRICS: &str = "
DELETE FROM agent_metrics WHERE ctid IN (
    SELECT ctid FROM (
        SELECT ctid, row_number() OVER (
            PARTITION BY agent_id, date_trunc('hour', timestamp) ORDER BY timestamp DESC
        ) AS keep_rank
        FROM agent_metrics WHERE timestamp < $1
    ) ranked WHERE keep_rank > 1 LIMIT 10000
)";

const EXPIRE_METRICS: &str = "
DELETE FROM agent_metrics WHERE ctid IN (
    SELECT ctid FROM agent_metrics WHERE timestamp < $1 LIMIT 10000
)";

const EXPIRE_JWTS: &str = "
DELETE FROM invalid_jwt WHERE ctid IN (
    SELECT ctid FROM invalid_jwt WHERE exp < $1 LIMIT 10000
)";

const EXPIRE_EVENTS: &str = "
DELETE FROM failover_events WHERE ctid IN (
    SELECT ctid FROM failover_events WHERE status <> 'open' AND created_at < $1 LIMIT 10000
)";

const EXPIRE_REVOCATIONS: &str = "
DELETE FROM certificate_revocations WHERE ctid IN (
    SELECT r.ctid FROM certificate_revocations r
    WHERE NOT EXISTS (
        SELECT 1 FROM agent_certificates c
        WHERE c.serial_number = r.serial_number AND c.expires_at > $1
    ) LIMIT 10000
)";

const EXPIRE_CERTIFICATES: &str = "
DELETE FROM agent_certificates WHERE ctid IN (
    SELECT ctid FROM agent_certificates WHERE expires_at < $1 LIMIT 10000
)";

const CLEAR_BUILD_LOGS: &str = "
UPDATE site_deployments SET build_log = '' WHERE ctid IN (
    SELECT ctid FROM site_deployments
    WHERE build_log <> '' AND COALESCE(finished_at, created_at) < $1 LIMIT 10000
)";

const EXPIRE_DEPLOYMENTS: &str = "
DELETE FROM site_deployments WHERE ctid IN (
    SELECT d.ctid FROM site_deployments d
    WHERE d.created_at < $1
      AND d.status NOT IN ('queued', 'building')
      AND d.artifact_key IS NULL
      AND d.image_ref IS NULL
      AND NOT EXISTS (SELECT 1 FROM sites s WHERE s.current_deployment_id = d.id)
    LIMIT 10000
)";

const ENSURE_AUDIT_PARTITIONS: &str = "
DO $$
DECLARE
    month DATE := date_trunc('month', now())::date;
BEGIN
    FOR step IN 0..3 LOOP
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS %I PARTITION OF vault_audit FOR VALUES FROM (%L) TO (%L)',
            'vault_audit_' || to_char(month, 'YYYY_MM'),
            month,
            (month + INTERVAL '1 month')::date
        );
        month := (month + INTERVAL '1 month')::date;
    END LOOP;
END
$$";

#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    pub metrics_raw_hours: i64,
    pub metrics_days: i64,
    pub events_days: i64,
    pub certificate_days: i64,
    pub build_log_days: i64,
    pub deployment_days: i64,
}

impl Policy {
    pub fn from_env() -> Result<Self, String> {
        let policy = Self {
            metrics_raw_hours: parsed("CSFX_RETENTION_METRICS_RAW_HOURS", 24)?,
            metrics_days: parsed("CSFX_RETENTION_METRICS_DAYS", 30)?,
            events_days: parsed("CSFX_RETENTION_EVENTS_DAYS", 90)?,
            certificate_days: parsed("CSFX_RETENTION_CERTIFICATE_DAYS", 30)?,
            build_log_days: parsed("CSFX_RETENTION_BUILD_LOG_DAYS", 14)?,
            deployment_days: parsed("CSFX_RETENTION_DEPLOYMENT_DAYS", 90)?,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), String> {
        let positive = [
            self.metrics_raw_hours,
            self.metrics_days,
            self.events_days,
            self.certificate_days,
            self.build_log_days,
            self.deployment_days,
        ]
        .iter()
        .all(|value| *value > 0);
        if !positive {
            return Err("retention values must be positive".to_string());
        }
        if self.metrics_raw_hours > self.metrics_days * 24 {
            return Err("raw metric retention exceeds total metric retention".to_string());
        }
        Ok(())
    }
}

struct Job {
    name: &'static str,
    sql: &'static str,
    cutoff: NaiveDateTime,
}

fn jobs(policy: &Policy, now: NaiveDateTime) -> Vec<Job> {
    let hours = |hours: i64| now - Span::hours(hours);
    let days = |days: i64| now - Span::days(days);
    vec![
        Job {
            name: "agent_metrics_thinned",
            sql: THIN_METRICS,
            cutoff: hours(policy.metrics_raw_hours),
        },
        Job {
            name: "agent_metrics_expired",
            sql: EXPIRE_METRICS,
            cutoff: days(policy.metrics_days),
        },
        Job {
            name: "invalid_jwt_expired",
            sql: EXPIRE_JWTS,
            cutoff: now,
        },
        Job {
            name: "failover_events_expired",
            sql: EXPIRE_EVENTS,
            cutoff: days(policy.events_days),
        },
        Job {
            name: "certificate_revocations_expired",
            sql: EXPIRE_REVOCATIONS,
            cutoff: now,
        },
        Job {
            name: "agent_certificates_expired",
            sql: EXPIRE_CERTIFICATES,
            cutoff: days(policy.certificate_days),
        },
        Job {
            name: "build_logs_cleared",
            sql: CLEAR_BUILD_LOGS,
            cutoff: days(policy.build_log_days),
        },
        Job {
            name: "site_deployments_expired",
            sql: EXPIRE_DEPLOYMENTS,
            cutoff: days(policy.deployment_days),
        },
    ]
}

async fn drain(db: &DatabaseConnection, job: &Job) -> Result<u64, DbErr> {
    let mut total = 0;
    loop {
        let statement =
            Statement::from_sql_and_values(db.get_database_backend(), job.sql, [job.cutoff.into()]);
        let affected = db.execute(statement).await?.rows_affected();
        total += affected;
        if affected < BATCH_ROWS {
            return Ok(total);
        }
    }
}

async fn ensure_audit_partitions(db: &DatabaseConnection) {
    if let Err(e) = db.execute_unprepared(ENSURE_AUDIT_PARTITIONS).await {
        crate::log_error!(
            "retention",
            &format!("Failed to create audit partitions err={}", e)
        );
    }
}

pub async fn run_once(db: &DatabaseConnection, policy: &Policy) {
    for job in jobs(policy, Utc::now().naive_utc()) {
        match drain(db, &job).await {
            Ok(0) => {}
            Ok(rows) => crate::log_info!(
                "retention",
                &format!("Retention applied job={} rows={}", job.name, rows)
            ),
            Err(e) => crate::log_error!(
                "retention",
                &format!("Retention failed job={} err={}", job.name, e)
            ),
        }
    }
    ensure_audit_partitions(db).await;
}

pub async fn run(db: DatabaseConnection, leader: LeaderLease, policy: Policy) {
    let mut tick = interval(RUN_INTERVAL);
    loop {
        tick.tick().await;
        if leader.is_leader() {
            run_once(&db, &policy).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        Policy {
            metrics_raw_hours: 24,
            metrics_days: 30,
            events_days: 90,
            certificate_days: 30,
            build_log_days: 14,
            deployment_days: 90,
        }
    }

    #[test]
    fn cutoffs_follow_the_policy() {
        let now = Utc::now().naive_utc();
        let planned = jobs(&policy(), now);
        let cutoff = |name: &str| planned.iter().find(|job| job.name == name).unwrap().cutoff;
        assert_eq!(cutoff("agent_metrics_thinned"), now - Span::hours(24));
        assert_eq!(cutoff("agent_metrics_expired"), now - Span::days(30));
        assert_eq!(cutoff("invalid_jwt_expired"), now);
        assert_eq!(cutoff("site_deployments_expired"), now - Span::days(90));
    }

    #[test]
    fn raw_retention_cannot_exceed_total_retention() {
        let mut invalid = policy();
        invalid.metrics_raw_hours = 24 * 30 + 1;
        assert!(invalid.validate().is_err());
        assert!(policy().validate().is_ok());
    }

    #[test]
    fn statements_batch_at_the_drain_size() {
        for job in jobs(&policy(), Utc::now().naive_utc()) {
            assert!(
                job.sql.contains(&format!("LIMIT {BATCH_ROWS}")),
                "{}",
                job.name
            );
        }
    }

    #[test]
    fn non_positive_values_are_rejected() {
        let mut invalid = policy();
        invalid.events_days = 0;
        assert!(invalid.validate().is_err());
    }
}
