use std::collections::HashMap;

use sea_orm::{
    ActiveValue::Set, ConnectionTrait, DbBackend, DbConn, EntityTrait, Statement, Value as DbValue,
};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{interval, Duration};
use uuid::Uuid;

use crate::db_log_layer::LogRecord;
use entity::logs;

const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const FLUSH_BATCH_SIZE: usize = 100;
const DEDUP_WINDOW_MINUTES: i32 = 10;
const KIND_SYSTEM: &str = "system";

const BUMP_EXISTING_SQL: &str = "UPDATE logs SET occurrences = occurrences + $1, last_seen_at = now() \
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

pub fn spawn_log_writer(mut receiver: UnboundedReceiver<LogRecord>, db: DbConn) {
    tokio::spawn(async move {
        let mut buffer = Vec::with_capacity(FLUSH_BATCH_SIZE);
        let mut ticker = interval(FLUSH_INTERVAL);

        loop {
            tokio::select! {
                record = receiver.recv() => {
                    let Some(record) = record else {
                        flush(&db, &mut buffer).await;
                        break;
                    };
                    buffer.push(record);
                    if buffer.len() >= FLUSH_BATCH_SIZE {
                        flush(&db, &mut buffer).await;
                    }
                }
                _ = ticker.tick() => {
                    flush(&db, &mut buffer).await;
                }
            }
        }
    });
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

async fn bump_existing(db: &DbConn, record: &LogRecord, count: i32) -> Result<bool, sea_orm::DbErr> {
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
