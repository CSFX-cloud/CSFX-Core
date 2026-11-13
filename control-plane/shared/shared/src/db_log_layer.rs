use serde_json::{Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, error::TrySendError, Receiver, Sender};
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::Id;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogClassification {
    Security,
    Performance,
    Audit,
    System,
    Network,
    Storage,
}

impl LogClassification {
    fn as_str(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Audit => "audit",
            Self::System => "system",
            Self::Network => "network",
            Self::Storage => "storage",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        [
            Self::Security,
            Self::Performance,
            Self::Audit,
            Self::System,
            Self::Network,
            Self::Storage,
        ]
        .into_iter()
        .find(|classification| classification.as_str() == name)
    }

    fn from_target(target: &str) -> Self {
        if target.contains("auth") || target.contains("rbac") || target.contains("jwt") {
            Self::Security
        } else if target.contains("scheduler") || target.contains("metrics") {
            Self::Performance
        } else if target.contains("network")
            || target.contains("sdn")
            || target.contains("wireguard")
        {
            Self::Network
        } else if target.contains("volume") || target.contains("storage") {
            Self::Storage
        } else {
            Self::System
        }
    }
}

pub const AUDIT_TARGET: &str = "csfx::audit";
const KIND_USER: &str = "user";
const KIND_SYSTEM: &str = "system";
const TRACKED_ID_KEYS: [&str; 3] = ["agent_id", "workload_id", "organization_id"];

pub struct LogRecord {
    pub service: &'static str,
    pub level: &'static str,
    pub classification: &'static str,
    pub kind: &'static str,
    pub message: String,
    pub agent_id: Option<Uuid>,
    pub workload_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub action: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub fields: Map<String, Value>,
}

#[derive(Default)]
struct EventVisitor {
    message: String,
    fields: Map<String, Value>,
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_value(field.name(), Value::String(format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field.name(), Value::String(value.to_string()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field.name(), Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field.name(), Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field.name(), Value::Bool(value));
    }
}

impl EventVisitor {
    fn record_value(&mut self, name: &str, value: Value) {
        if name != "message" {
            self.fields.insert(name.to_string(), value);
            return;
        }
        if let Value::String(message) = value {
            self.message = message;
        }
    }

    fn take_string(&mut self, name: &str) -> Option<String> {
        match self.fields.remove(name)? {
            Value::String(text) => Some(text),
            other => Some(other.to_string()),
        }
    }

    fn take_uuid(&mut self, name: &str) -> Option<Uuid> {
        self.take_string(name)
            .and_then(|text| Uuid::parse_str(&text).ok())
    }
}

fn uuid_from_message(message: &str, key: &str) -> Option<Uuid> {
    let needle = format!("{key}=");
    let start = message.find(&needle)? + needle.len();
    let token = message[start..].split_whitespace().next()?;
    Uuid::parse_str(token.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != '-')).ok()
}

fn level_as_str(level: &Level) -> &'static str {
    match *level {
        Level::TRACE | Level::DEBUG => "DEBUG",
        Level::INFO => "INFO",
        Level::WARN => "WARN",
        Level::ERROR => "ERROR",
    }
}

pub const LOG_CHANNEL_CAPACITY: usize = 10_000;
pub const AUDIT_CHANNEL_CAPACITY: usize = 10_000;

pub struct LogReceiver {
    pub(crate) records: Receiver<LogRecord>,
    pub(crate) audit: Receiver<LogRecord>,
    pub(crate) dropped: Arc<AtomicU64>,
}

struct LogSink {
    records: Sender<LogRecord>,
    audit: Sender<LogRecord>,
    dropped: Arc<AtomicU64>,
}

impl LogSink {
    fn submit(&self, record: LogRecord) {
        let sender = if record.kind == KIND_USER {
            &self.audit
        } else {
            &self.records
        };
        if let Err(TrySendError::Full(_)) = sender.try_send(record) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub struct DbLogLayer {
    service: &'static str,
    sink: LogSink,
}

impl DbLogLayer {
    pub fn new(service: &'static str) -> (Self, LogReceiver) {
        let (records_sender, records) = channel(LOG_CHANNEL_CAPACITY);
        let (audit_sender, audit) = channel(AUDIT_CHANNEL_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let sink = LogSink {
            records: records_sender,
            audit: audit_sender,
            dropped: Arc::clone(&dropped),
        };
        let receiver = LogReceiver {
            records,
            audit,
            dropped,
        };
        (Self { service, sink }, receiver)
    }
}

const NOISY_ACCESS_TARGET: &str = "csfx::http_access";
const NOISY_TARGET_PREFIXES: [&str; 2] = ["sea_orm", "sqlx"];
const NOISY_MESSAGE_SUBSTRINGS: [&str; 1] = ["heartbeat processed"];

fn is_routine_access_event(event: &Event<'_>) -> bool {
    event.metadata().target() == NOISY_ACCESS_TARGET && *event.metadata().level() == Level::INFO
}

fn is_sql_trace_event(event: &Event<'_>) -> bool {
    let target = event.metadata().target();
    NOISY_TARGET_PREFIXES
        .iter()
        .any(|prefix| target.starts_with(prefix))
}

fn is_performance_event(event: &Event<'_>) -> bool {
    *event.metadata().level() == Level::INFO
        && LogClassification::from_target(event.metadata().target())
            == LogClassification::Performance
}

fn is_noisy_message_event(visitor: &EventVisitor) -> bool {
    NOISY_MESSAGE_SUBSTRINGS
        .iter()
        .any(|needle| visitor.message.contains(needle))
}

fn build_record(service: &'static str, event: &Event<'_>, mut visitor: EventVisitor) -> LogRecord {
    let target = event.metadata().target();
    let is_audit = target == AUDIT_TARGET;
    let classification = visitor
        .take_string("classification")
        .and_then(|name| LogClassification::from_name(&name))
        .unwrap_or(if is_audit {
            LogClassification::Audit
        } else {
            LogClassification::from_target(target)
        });
    let [agent_id, workload_id, organization_id] = TRACKED_ID_KEYS.map(|key| {
        visitor
            .take_uuid(key)
            .or_else(|| uuid_from_message(&visitor.message, key))
    });

    LogRecord {
        service,
        level: level_as_str(event.metadata().level()),
        classification: classification.as_str(),
        kind: if is_audit { KIND_USER } else { KIND_SYSTEM },
        agent_id,
        workload_id,
        organization_id,
        actor_user_id: visitor.take_uuid("actor_user_id"),
        action: visitor.take_string("action"),
        target_type: visitor.take_string("target_type"),
        target_id: visitor.take_string("target_id"),
        message: visitor.message,
        fields: visitor.fields,
    }
}

const REQUEST_ID_KEY: &str = "request_id";

struct RequestId(String);

#[derive(Default)]
struct RequestIdVisitor(Option<String>);

impl Visit for RequestIdVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == REQUEST_ID_KEY {
            self.0 = Some(format!("{value:?}"));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == REQUEST_ID_KEY {
            self.0 = Some(value.to_string());
        }
    }
}

fn request_id_in_scope<S>(event: &Event<'_>, ctx: &Context<'_, S>) -> Option<String>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    ctx.event_scope(event)?
        .find_map(|span| span.extensions().get::<RequestId>().map(|id| id.0.clone()))
}

impl<S> Layer<S> for DbLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut visitor = RequestIdVisitor::default();
        attrs.record(&mut visitor);
        let Some(request_id) = visitor.0 else {
            return;
        };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(RequestId(request_id));
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if *event.metadata().level() == Level::TRACE {
            return;
        }

        if is_routine_access_event(event)
            || is_sql_trace_event(event)
            || is_performance_event(event)
        {
            return;
        }

        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);

        if is_noisy_message_event(&visitor) {
            return;
        }

        if let Some(request_id) = request_id_in_scope(event, &ctx) {
            visitor
                .fields
                .entry(REQUEST_ID_KEY)
                .or_insert(Value::String(request_id));
        }

        self.sink.submit(build_record(self.service, event, visitor));
    }
}

#[macro_export]
macro_rules! audit {
    ($actor:expr, $action:expr, $target_type:expr, $target_id:expr $(, $key:ident = $value:expr)* $(,)?) => {
        tracing::info!(
            target: "csfx::audit",
            actor_user_id = %$actor,
            action = $action,
            target_type = $target_type,
            target_id = %$target_id,
            $($key = %$value,)*
            "{}",
            $action
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: &'static str) -> LogRecord {
        LogRecord {
            service: "test",
            level: "INFO",
            classification: "system",
            kind,
            message: "entry".to_string(),
            agent_id: None,
            workload_id: None,
            organization_id: None,
            actor_user_id: None,
            action: None,
            target_type: None,
            target_id: None,
            fields: Map::new(),
        }
    }

    fn drain(receiver: &mut Receiver<LogRecord>) -> usize {
        std::iter::from_fn(|| receiver.try_recv().ok()).count()
    }

    #[test]
    fn overflow_without_consumer_stays_bounded_and_counts_drops() {
        let (layer, mut receiver) = DbLogLayer::new("test");
        let extra = 500;
        for _ in 0..LOG_CHANNEL_CAPACITY + extra {
            layer.sink.submit(record(KIND_SYSTEM));
        }

        assert_eq!(receiver.dropped.load(Ordering::Relaxed), extra as u64);
        assert_eq!(drain(&mut receiver.records), LOG_CHANNEL_CAPACITY);
    }

    #[test]
    fn audit_records_are_not_starved_by_log_floods() {
        let (layer, mut receiver) = DbLogLayer::new("test");
        for _ in 0..LOG_CHANNEL_CAPACITY + 1 {
            layer.sink.submit(record(KIND_SYSTEM));
        }
        layer.sink.submit(record(KIND_USER));

        assert_eq!(drain(&mut receiver.audit), 1);
        assert_eq!(receiver.dropped.load(Ordering::Relaxed), 1);
    }
}
