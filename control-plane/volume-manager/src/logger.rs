use tracing::{event, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

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

pub fn init_logger() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false).with_thread_ids(true))
        .init();
}

pub fn log_message(level: LogLevel, module: &str, location: &str, description: &str) {
    let lvl: Level = level.into();

    match lvl {
        Level::ERROR => {
            event!(Level::ERROR, module = %module, location = %location, "{}", description)
        }
        Level::WARN => {
            event!(Level::WARN,  module = %module, location = %location, "{}", description)
        }
        Level::INFO => {
            event!(Level::INFO,  module = %module, location = %location, "{}", description)
        }
        Level::DEBUG => {
            event!(Level::DEBUG, module = %module, location = %location, "{}", description)
        }
        Level::TRACE => {
            event!(Level::TRACE, module = %module, location = %location, "{}", description)
        }
    }
}
