use tracing::{error, info, warn};

pub fn init_logger() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();
    info!("Logger initialized");
}
