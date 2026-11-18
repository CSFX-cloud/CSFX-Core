use shared::leader::LeaderLease;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

use scheduler::{db as store, log_error, log_info, logger, server, services};

const LEADER_KEY: &str = "/csfx/leaders/scheduler";

fn spawn_pending_retry(scheduler: Arc<services::scheduler::SchedulerService>, leader: LeaderLease) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            if !leader.is_leader() {
                continue;
            }
            match scheduler.retry_pending().await {
                Ok(0) => {}
                Ok(count) => {
                    log_info!(
                        "main",
                        &format!("Retried pending workloads placed={}", count)
                    );
                }
                Err(e) => {
                    log_error!("main", &format!("Pending retry failed err={}", e));
                }
            }
        }
    });
}

fn spawn_replacement_reconcile(
    scheduler: Arc<services::scheduler::SchedulerService>,
    leader: LeaderLease,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            if !leader.is_leader() {
                continue;
            }
            if let Err(e) = scheduler.reconcile_replacements().await {
                log_error!("main", &format!("Replacement reconcile failed err={}", e));
            }
        }
    });
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    dotenvy::dotenv().ok();

    let log_receiver = logger::init_logger();

    log_info!("main", "CSFX Scheduler Service starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    shared::env_secrets::init().expect("Failed to load environment encryption key");

    log_info!("main", "Connecting to database...");
    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");
    shared::spawn_log_writer(log_receiver, db.clone());
    let workload_db = db.clone();
    shared::env_secrets::spawn_backfill("workload_env", move || {
        let db = workload_db.clone();
        async move { store::workloads::seal_legacy_env(&db).await }
    });
    let stack_db = db.clone();
    shared::env_secrets::spawn_backfill("stack_sources", move || {
        let db = stack_db.clone();
        async move { store::workload_stacks::seal_legacy_sources(&db).await }
    });
    shared::observability::spawn_pool_gauges("default", db.clone());

    let etcd_endpoints = shared::etcd::endpoints_from_env();

    log_info!(
        "main",
        &format!("Connecting to etcd endpoints={}", etcd_endpoints.join(","))
    );
    let etcd = etcd_client::Client::connect(&etcd_endpoints, None)
        .await
        .expect("Failed to connect to etcd");
    log_info!("main", "etcd connection established");

    let etcd = Arc::new(Mutex::new(etcd));
    let scheduler = Arc::new(services::scheduler::SchedulerService::new(
        db.clone(),
        etcd.clone(),
    ));

    let state = server::AppState {
        db,
        etcd,
        scheduler,
    };

    let app = server::create_router(state.clone());

    let leader = LeaderLease::connect(LEADER_KEY).await?;
    spawn_replacement_reconcile(state.scheduler.clone(), leader.clone());
    spawn_pending_retry(state.scheduler.clone(), leader);

    match state.scheduler.backfill_guest_ips().await {
        Ok(0) => {}
        Ok(count) => log_info!("main", &format!("Backfilled guest ips count={}", count)),
        Err(e) => log_error!("main", &format!("Guest ip backfill failed err={}", e)),
    }

    shared::observability::serve_metrics("127.0.0.1:9002").await;

    let port = std::env::var("SCHEDULER_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8002);

    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: SocketAddr = format!("{}:{}", listen_addr, port).parse().unwrap();
    log_info!("main", &format!("Scheduler listening port={}", port));

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                log_error!("main", &format!("Server error err={}", e));
            }
        }
        _ = tokio::signal::ctrl_c() => {
            log_info!("main", "Shutdown signal received");
        }
    }

    log_info!("main", "Scheduler Service shutting down");
    Ok(())
}
