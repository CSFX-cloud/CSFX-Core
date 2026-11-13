use failover_controller::services::retention::{run_once, Policy};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use crate::common::{ago, database, insert_agent, insert_metric, STALE_LOCK};

const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;

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

async fn run(db: &DatabaseConnection, sql: String) {
    db.execute_unprepared(&sql).await.expect("statement failed");
}

async fn count(db: &DatabaseConnection, sql: String) -> i64 {
    db.query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index(0)
        .unwrap()
}

fn stamp(seconds: i64) -> String {
    format!("'{}'", ago(seconds).format("%Y-%m-%d %H:%M:%S"))
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn metrics_are_thinned_after_the_raw_window_and_dropped_after_the_limit() {
    let _guard = STALE_LOCK.lock().await;
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    let hour_start = 48 * HOUR + 600;
    for offset in [0, 60, 120] {
        insert_metric(&db, agent.id, (1.0, 1.0, 1.0), hour_start + offset).await;
    }
    insert_metric(&db, agent.id, (1.0, 1.0, 1.0), 40 * DAY).await;
    insert_metric(&db, agent.id, (1.0, 1.0, 1.0), 2 * HOUR).await;
    insert_metric(&db, agent.id, (1.0, 1.0, 1.0), 2 * HOUR + 60).await;

    run_once(&db, &policy()).await;

    let remaining = |window: &str| {
        format!(
            "SELECT COUNT(*) FROM agent_metrics WHERE agent_id = '{}' AND {window}",
            agent.id
        )
    };
    assert_eq!(
        count(&db, remaining("timestamp < now() - interval '30 days'")).await,
        0
    );
    assert_eq!(
        count(
            &db,
            remaining(
                "timestamp BETWEEN now() - interval '49 hours' AND now() - interval '47 hours'"
            )
        )
        .await,
        1
    );
    assert_eq!(
        count(&db, remaining("timestamp > now() - interval '3 hours'")).await,
        2
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn expired_tokens_and_closed_events_are_removed() {
    let _guard = STALE_LOCK.lock().await;
    let db = database().await;
    let marker = Uuid::new_v4();
    run(&db, format!(
        "INSERT INTO invalid_jwt (id, token, exp) VALUES ('{}', 'old-{marker}', {}), ('{}', 'new-{marker}', {})",
        Uuid::new_v4(), stamp(HOUR), Uuid::new_v4(), stamp(-HOUR)
    )).await;
    let event = |status: &str, age: i64| {
        format!(
            "('{}', 'retention-{marker}', 'warning', '{status}', {})",
            Uuid::new_v4(),
            stamp(age)
        )
    };
    run(&db, format!(
        "INSERT INTO failover_events (id, event_type, severity, status, created_at) VALUES {}, {}, {}",
        event("resolved", 100 * DAY), event("open", 100 * DAY), event("resolved", DAY)
    )).await;

    run_once(&db, &policy()).await;

    assert_eq!(
        count(
            &db,
            format!("SELECT COUNT(*) FROM invalid_jwt WHERE token LIKE '%-{marker}'")
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &db,
            format!("SELECT COUNT(*) FROM failover_events WHERE event_type = 'retention-{marker}'")
        )
        .await,
        2
    );
    assert_eq!(count(&db, format!("SELECT COUNT(*) FROM failover_events WHERE event_type = 'retention-{marker}' AND status = 'resolved' AND created_at < now() - interval '2 days'")).await, 0);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn certificates_and_their_revocations_expire_together() {
    let _guard = STALE_LOCK.lock().await;
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    let base = i64::from(Uuid::new_v4().as_u128() as u32) * 10;
    let certificate = |serial: i64, expires_in: i64| {
        format!(
            "('{}', '{}', {serial}, 'pem', 'key', {}, {}, false)",
            Uuid::new_v4(),
            agent.id,
            stamp(60 * DAY),
            stamp(-expires_in)
        )
    };
    run(&db, format!(
        "INSERT INTO agent_certificates (id, agent_id, serial_number, certificate_pem, public_key_pem, issued_at, expires_at, is_active) VALUES {}, {}, {}",
        certificate(base, -40 * DAY), certificate(base + 1, -DAY), certificate(base + 2, 10 * DAY)
    )).await;
    let revocation = |serial: i64| {
        format!(
            "('{}', {serial}, '{}', {}, 'test')",
            Uuid::new_v4(),
            agent.id,
            stamp(DAY)
        )
    };
    run(&db, format!(
        "INSERT INTO certificate_revocations (id, serial_number, agent_id, revoked_at, reason) VALUES {}, {}, {}",
        revocation(base), revocation(base + 1), revocation(base + 2)
    )).await;

    run_once(&db, &policy()).await;

    let serials = format!("{base}, {}, {}", base + 1, base + 2);
    let certificates =
        format!("SELECT COUNT(*) FROM agent_certificates WHERE serial_number IN ({serials})");
    let revocations =
        format!("SELECT COUNT(*) FROM certificate_revocations WHERE serial_number IN ({serials})");
    assert_eq!(count(&db, certificates).await, 2);
    assert_eq!(count(&db, revocations).await, 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn old_deployments_lose_their_logs_and_rows_but_not_live_ones() {
    let _guard = STALE_LOCK.lock().await;
    let db = database().await;
    let org = Uuid::new_v4();
    let group = Uuid::new_v4();
    let site = Uuid::new_v4();
    let current = Uuid::new_v4();
    let octets = org.as_bytes();
    let cidr = format!("10.{}.{}.0/24", octets[0], octets[1]);
    run(&db, format!("INSERT INTO organization (id, name, created_at, updated_at) VALUES ('{org}', 'retention-{org}', now(), now())")).await;
    run(&db, format!("INSERT INTO resource_groups (id, organization_id, name, internal_cidr, status, icon, color, pinned, created_at) VALUES ('{group}', '{org}', 'retention', '{cidr}', 'active', 'box', 'blue', false, now())")).await;
    run(&db, format!("INSERT INTO sites (id, resource_group_id, name, mode, repo_url, webhook_secret, current_deployment_id, created_at) VALUES ('{site}', '{group}', 'retention', 'static', 'https://example.invalid/repo', 'secret', '{current}', now())")).await;
    let deployment = |id: Uuid, status: &str, artifact: &str, age: i64, log: &str| {
        format!(
            "('{id}', '{site}', 'sha', 'main', 'production', '{status}', {artifact}, {}, {}, '{log}')",
            stamp(age),
            stamp(age)
        )
    };
    let (old_bare, old_kept, old_current, recent, building) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        current,
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    run(&db, format!(
        "INSERT INTO site_deployments (id, site_id, commit_sha, branch, environment, status, artifact_key, created_at, finished_at, build_log) VALUES {}, {}, {}, {}, {}",
        deployment(old_bare, "failed", "NULL", 100 * DAY, "log"),
        deployment(old_kept, "ready", "'key'", 100 * DAY, "log"),
        deployment(old_current, "ready", "NULL", 100 * DAY, "log"),
        deployment(recent, "ready", "NULL", 10 * DAY, "log"),
        deployment(building, "building", "NULL", 100 * DAY, "log"),
    )).await;

    run_once(&db, &policy()).await;

    let present = |id: Uuid| format!("SELECT COUNT(*) FROM site_deployments WHERE id = '{id}'");
    let cleared = |id: Uuid| {
        format!("SELECT COUNT(*) FROM site_deployments WHERE id = '{id}' AND build_log = ''")
    };
    let removed_bare = count(&db, present(old_bare)).await;
    let mut kept = Vec::new();
    for id in [old_kept, old_current, recent, building] {
        kept.push(count(&db, present(id)).await);
    }
    let old_cleared = count(&db, cleared(old_kept)).await;
    let recent_cleared = count(&db, cleared(recent)).await;
    run(&db, format!("DELETE FROM organization WHERE id = '{org}'")).await;

    assert_eq!(removed_bare, 0);
    assert_eq!(kept, vec![1, 1, 1, 1]);
    assert_eq!(old_cleared, 1);
    assert_eq!(recent_cleared, 0);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn audit_rows_stay_append_only_and_land_in_monthly_partitions() {
    let db = database().await;
    run_once(&db, &policy()).await;
    let vault = Uuid::new_v4();
    run(&db, format!("INSERT INTO vault_audit (id, vault_id, occurred_at, actor, action, target, source, result) VALUES ('{}', '{vault}', now(), 'a', 'read', 't', 's', 'ok')", Uuid::new_v4())).await;

    let partition = format!(
        "SELECT COUNT(*) FROM vault_audit_{} WHERE vault_id = '{vault}'",
        chrono::Utc::now().format("%Y_%m")
    );
    assert_eq!(count(&db, partition).await, 1);
    assert!(db
        .execute_unprepared(&format!(
            "DELETE FROM vault_audit WHERE vault_id = '{vault}'"
        ))
        .await
        .is_err());
    assert!(db
        .execute_unprepared(&format!(
            "UPDATE vault_audit SET actor = 'x' WHERE vault_id = '{vault}'"
        ))
        .await
        .is_err());
}
