use shared::leader::LeaderLease;
use std::net::SocketAddr;
use uuid::Uuid;

mod crypto;
mod db;
mod garage;
mod handlers;
mod logger;
mod metrics;
mod models;
mod server;
mod services;

const LAYOUT_LOCK_KEY: &str = "/csfx/object-storage/layout-lock";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    dotenvy::dotenv().ok();

    let log_receiver = logger::init_logger();

    metrics::init();
    log_info!("main", "CSFX Object Storage starting...");
    log_info!("main", &format!("Version: {}", env!("CARGO_PKG_VERSION")));

    log_info!("main", "Connecting to database...");
    let db = shared::establish_connection()
        .await
        .expect("Failed to connect to database");
    log_info!("main", "Database connection established");
    shared::spawn_log_writer(log_receiver, db.clone());

    let admin_url =
        std::env::var("GARAGE_ADMIN_URL").unwrap_or_else(|_| "http://127.0.0.1:3903".to_string());
    let admin_token = std::env::var("GARAGE_ADMIN_TOKEN").expect("GARAGE_ADMIN_TOKEN must be set");
    let garage = garage::GarageClient::new(admin_url, admin_token);

    let s3_url =
        std::env::var("GARAGE_S3_URL").unwrap_or_else(|_| "http://127.0.0.1:3900".to_string());
    let public_s3_url = std::env::var("GARAGE_PUBLIC_S3_URL").unwrap_or_else(|_| s3_url.clone());
    let s3_client = garage::S3Client::new(s3_url, public_s3_url);

    let etcd_url =
        std::env::var("ETCD_URL").unwrap_or_else(|_| "http://localhost:2379".to_string());
    let etcd = etcd_client::Client::connect([etcd_url.as_str()], None)
        .await
        .expect("Failed to connect to etcd");
    log_info!("main", "etcd connection established");

    let node_id = Uuid::new_v4().to_string();
    let leader = LeaderLease::new(etcd, LAYOUT_LOCK_KEY, node_id).spawn();

    tokio::spawn(garage::layout::run_reconcile_loop(
        db.clone(),
        garage.clone(),
        leader,
    ));

    let self_register_zone = std::env::var("GARAGE_ZONE").unwrap_or_else(|_| "dev".to_string());
    let self_register_data_dir =
        std::env::var("GARAGE_DATA_DIR").unwrap_or_else(|_| "/var/lib/garage/data".to_string());
    tokio::spawn({
        let db = db.clone();
        let garage = garage.clone();
        async move {
            garage::layout::register_self_as_node(
                &db,
                &garage,
                &self_register_zone,
                &self_register_data_dir,
            )
            .await;
        }
    });

    let secret_box = std::sync::Arc::new(
        crypto::SecretBox::from_env().expect("Failed to initialize encryption key"),
    );

    let state = server::AppState::new(db, garage, s3_client, secret_box);
    let app = server::create_router(state);

    let port = std::env::var("OBJECT_STORAGE_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8006);

    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: SocketAddr = format!("{}:{}", listen_addr, port).parse().unwrap();
    log_info!("main", &format!("Object Storage listening port={}", port));

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

    log_info!("main", "Object Storage shutting down");
    Ok(())
}
