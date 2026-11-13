use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use sea_orm::{
    ActiveValue::Set, ConnectionTrait, DbBackend, DbConn, EntityTrait, Statement, Value as DbValue,
};
use tokio::time::{interval, Duration};
use uuid::Uuid;

use crate::db_log_layer::{LogReceiver, LogRecord};
use entity::{audit_log, logs};

const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const FLUSH_BATCH_SIZE: usize = 100;
const DEDUP_WINDOW_MINUTES: i32 = 10;
const KIND_SYSTEM: &str = "system";
const AUDIT_BUFFER_LIMIT: usize = 1_000;

const BUMP_EXISTING_SQL: &str =
    "UPDATE logs SET occurrences = occurrences + $1, last_seen_at = now() \
     WHERE id = (SELECT id FROM logs WHERE kind = 'system' AND service = $2 AND level = $3 \
     AND message = $4 AND agent_id IS NOT DISTINCT FROM $5 \
     AND last_seen_at > now() - make_interval(mins => $6) \
     ORDER BY last_seen_at DESC LIMIT 1)";

#[derive(PartialEq, Eq, Hash)]
struct DedupKey {
    service: &'static str,
    level: &'static str,
    message: String,
    agent_id: Option<Uuid>,
}

pub fn spawn_log_writer(mut receiver: LogReceiver, db: DbConn) {
    tokio::spawn(async move {
        let mut buffer = Vec::with_capacity(FLUSH_BATCH_SIZE);
        let mut audit_buffer = Vec::with_capacity(FLUSH_BATCH_SIZE);
        let mut ticker = interval(FLUSH_INTERVAL);

        loop {
            tokio::select! {
                record = receiver.records.recv() => {
                    let Some(record) = record else { break };
                    buffer.push(record);
                    if buffer.len() >= FLUSH_BATCH_SIZE {
                        flush(&db, &mut buffer).await;
                    }
                }
                record = receiver.audit.recv(), if audit_buffer.len() < AUDIT_BUFFER_LIMIT => {
                    let Some(record) = record else { break };
                    audit_buffer.push(record);
                    if audit_buffer.len() >= FLUSH_BATCH_SIZE {
                        flush_audit(&db, &mut audit_buffer).await;
                    }
                }
                _ = ticker.tick() => {
                    flush(&db, &mut buffer).await;
                    flush_audit(&db, &mut audit_buffer).await;
                    report_dropped(&receiver.dropped);
                }
            }
        }
        flush(&db, &mut buffer).await;
        flush_audit(&db, &mut audit_buffer).await;
    });
}

fn report_dropped(dropped: &AtomicU64) {
    let count = dropped.swap(0, Ordering::Relaxed);
    if count > 0 {
        tracing::warn!(target: "shared::log_writer", dropped = count, "log records dropped, channel full");
    }
}

fn is_deduplicable(record: &LogRecord) -> bool {
    record.kind == KIND_SYSTEM && matches!(record.level, "WARN" | "ERROR")
}

fn group_duplicates(buffer: &mut Vec<LogRecord>) -> (Vec<(LogRecord, i32)>, Vec<LogRecord>) {
    let mut grouped: HashMap<DedupKey, (LogRecord, i32)> = HashMap::new();
    let mut unique = Vec::new();

    for record in buffer.drain(..) {
        if !is_deduplicable(&record) {
            unique.push(record);
            continue;
        }
        let key = DedupKey {
            service: record.service,
            level: record.level,
            message: record.message.clone(),
            agent_id: record.agent_id,
        };
        grouped.entry(key).or_insert((record, 0)).1 += 1;
    }

    (grouped.into_values().collect(), unique)
}

async fn bump_existing(
    db: &DbConn,
    record: &LogRecord,
    count: i32,
) -> Result<bool, sea_orm::DbErr> {
    let statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        BUMP_EXISTING_SQL,
        [
            DbValue::from(count),
            DbValue::from(record.service),
            DbValue::from(record.level),
            DbValue::from(record.message.clone()),
            DbValue::from(record.agent_id),
            DbValue::from(DEDUP_WINDOW_MINUTES),
        ],
    );
    Ok(db.execute(statement).await?.rows_affected() > 0)
}

fn to_model(record: LogRecord, occurrences: i32) -> logs::ActiveModel {
    let now = chrono::Utc::now().into();
    logs::ActiveModel {
        id: Set(Uuid::new_v4()),
        service: Set(record.service.to_string()),
        level: Set(record.level.to_string()),
        classification: Set(record.classification.to_string()),
        message: Set(record.message),
        agent_id: Set(record.agent_id),
        workload_id: Set(record.workload_id),
        organization_id: Set(record.organization_id),
        created_at: Set(now),
        kind: Set(record.kind.to_string()),
        actor_user_id: Set(record.actor_user_id),
        action: Set(record.action),
        target_type: Set(record.target_type),
        target_id: Set(record.target_id),
        fields: Set(serde_json::Value::Object(record.fields)),
        occurrences: Set(occurrences),
        last_seen_at: Set(now),
    }
}

async fn flush(db: &DbConn, buffer: &mut Vec<LogRecord>) {
    if buffer.is_empty() {
        return;
    }

    let (duplicates, unique) = group_duplicates(buffer);
    let mut models: Vec<logs::ActiveModel> = unique.into_iter().map(|r| to_model(r, 1)).collect();

    for (record, count) in duplicates {
        match bump_existing(db, &record, count).await {
            Ok(true) => {}
            Ok(false) => models.push(to_model(record, count)),
            Err(error) => {
                tracing::warn!(target: "shared::log_writer", error = %error, "failed to bump log occurrences");
                models.push(to_model(record, count));
            }
        }
    }

    if models.is_empty() {
        return;
    }

    if let Err(error) = logs::Entity::insert_many(models).exec(db).await {
        tracing::warn!(target: "shared::log_writer", error = %error, "failed to flush logs to database");
    }
}

fn to_audit_model(record: &LogRecord) -> audit_log::ActiveModel {
    let now = chrono::Utc::now().into();
    audit_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        service: Set(record.service.to_string()),
        level: Set(record.level.to_string()),
        classification: Set(record.classification.to_string()),
        message: Set(record.message.clone()),
        agent_id: Set(record.agent_id),
        workload_id: Set(record.workload_id),
        organization_id: Set(record.organization_id),
        created_at: Set(now),
        kind: Set(record.kind.to_string()),
        actor_user_id: Set(record.actor_user_id),
        action: Set(record.action.clone()),
        target_type: Set(record.target_type.clone()),
        target_id: Set(record.target_id.clone()),
        fields: Set(serde_json::Value::Object(record.fields.clone())),
        occurrences: Set(1),
        last_seen_at: Set(now),
    }
}

async fn flush_audit(db: &DbConn, buffer: &mut Vec<LogRecord>) {
    if buffer.is_empty() {
        return;
    }
    let models = buffer.iter().map(to_audit_model);
    match audit_log::Entity::insert_many(models).exec(db).await {
        Ok(_) => buffer.clear(),
        Err(error) => {
            tracing::warn!(target: "shared::log_writer", error = %error, pending = buffer.len(), "failed to flush audit log, retrying");
        }
    }
}
