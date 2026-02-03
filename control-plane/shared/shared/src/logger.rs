use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the tracing logger with environment filter support
///
/// Usage:
/// - Set RUST_LOG env variable to control log level (e.g., RUST_LOG=debug)
/// - Default level is "info"
pub fn init_logger() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_init() {
        // This test just verifies that logger initialization doesn't panic
        init_logger();
    }
}
