use prometheus::{register_gauge, register_gauge_vec, Gauge, GaugeVec};
use sea_orm::DatabaseConnection;
use std::sync::LazyLock;
use std::time::Duration;

const POOL_SAMPLE_INTERVAL: Duration = Duration::from_secs(15);

static POOL_CONNECTIONS: LazyLock<GaugeVec> = LazyLock::new(|| {
    register_gauge_vec!(
        "csfx_db_pool_connections",
        "Open database pool connections",
        &["pool"]
    )
    .expect("failed to register csfx_db_pool_connections")
});

static POOL_IDLE_CONNECTIONS: LazyLock<GaugeVec> = LazyLock::new(|| {
    register_gauge_vec!(
        "csfx_db_pool_idle_connections",
        "Idle database pool connections",
        &["pool"]
    )
    .expect("failed to register csfx_db_pool_idle_connections")
});

static POOL_MAX_CONNECTIONS: LazyLock<GaugeVec> = LazyLock::new(|| {
    register_gauge_vec!(
        "csfx_db_pool_max_connections",
        "Configured maximum database pool connections",
        &["pool"]
    )
    .expect("failed to register csfx_db_pool_max_connections")
});

static AGENT_HEARTBEAT_LAG_SECONDS: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge!(
        "csfx_agent_heartbeat_lag_seconds_max",
        "Seconds since the oldest heartbeat among live agents"
    )
    .expect("failed to register csfx_agent_heartbeat_lag_seconds_max")
});

pub fn set_agent_heartbeat_lag_seconds(seconds: f64) {
    AGENT_HEARTBEAT_LAG_SECONDS.set(seconds);
}

pub fn sample_pool(pool: &str, db: &DatabaseConnection) {
    let pg = db.get_postgres_connection_pool();
    POOL_CONNECTIONS
        .with_label_values(&[pool])
        .set(f64::from(pg.size()));
    POOL_IDLE_CONNECTIONS
        .with_label_values(&[pool])
        .set(pg.num_idle() as f64);
    POOL_MAX_CONNECTIONS
        .with_label_values(&[pool])
        .set(f64::from(pg.options().get_max_connections()));
}

pub fn spawn_pool_gauges(pool: &'static str, db: DatabaseConnection) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POOL_SAMPLE_INTERVAL);
        loop {
            interval.tick().await;
            sample_pool(pool, &db);
        }
    });
}
