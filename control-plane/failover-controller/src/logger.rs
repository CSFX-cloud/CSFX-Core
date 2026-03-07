use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::{event, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

fn build_otlp_provider(service_name: &str) -> Option<SdkTracerProvider> {
    let endpoint = std::env::var("OTLP_ENDPOINT").ok()?;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .ok()?;

    let resource = opentelemetry_sdk::Resource::builder_empty()
        .with_attribute(KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAME,
            service_name.to_string(),
        ))
        .build();

    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build();

    Some(provider)
}

pub fn init_logger() {
    init_logger_with_service(env!("CARGO_PKG_NAME"));
}

pub fn init_logger_with_service(service_name: &'static str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().with_target(false).with_thread_ids(true);

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer);

    match build_otlp_provider(service_name) {
        Some(provider) => {
            let tracer = provider.tracer(service_name);
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            registry.with(otel_layer).init();
            tracing::info!(service = service_name, "OpenTelemetry tracing enabled");
        }
        None => {
            registry.init();
        }
    }
}

pub fn log_message(level: LogLevel, module: &str, location: &str, description: &str) {
    let lvl: Level = level.into();

    match lvl {
        Level::ERROR => event!(Level::ERROR, module = %module, location = %location, "{}", description),
        Level::WARN  => event!(Level::WARN,  module = %module, location = %location, "{}", description),
        Level::INFO  => event!(Level::INFO,  module = %module, location = %location, "{}", description),
        Level::DEBUG => event!(Level::DEBUG, module = %module, location = %location, "{}", description),
        Level::TRACE => event!(Level::TRACE, module = %module, location = %location, "{}", description),
    }
}

#[macro_export]
macro_rules! log_info {
    ($module:expr, $desc:expr) => {
        $crate::logger::log_message(
            $crate::logger::LogLevel::Info,
            $module,
            concat!(file!(), ":", line!()),
            $desc,
        )
    };
}

#[macro_export]
macro_rules! log_warn {
    ($module:expr, $desc:expr) => {
        $crate::logger::log_message(
            $crate::logger::LogLevel::Warn,
            $module,
            concat!(file!(), ":", line!()),
            $desc,
        )
    };
}

#[macro_export]
macro_rules! log_error {
    ($module:expr, $desc:expr) => {
        $crate::logger::log_message(
            $crate::logger::LogLevel::Error,
            $module,
            concat!(file!(), ":", line!()),
            $desc,
        )
    };
}

#[macro_export]
macro_rules! log_debug {
    ($module:expr, $desc:expr) => {
        $crate::logger::log_message(
            $crate::logger::LogLevel::Debug,
            $module,
            concat!(file!(), ":", line!()),
            $desc,
        )
    };
}
