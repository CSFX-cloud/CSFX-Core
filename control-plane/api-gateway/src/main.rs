use api_gateway::{
    db, ingress_sync, init, metrics, openbao, prune, recovery, routes, service_client, site_client,
    telemetry, tls, vault_service, ApiDoc, AppState,
};
use std::net::SocketAddr;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

async fn connect_or_exit(name: &str, default_max_connections: u32) -> sea_orm::DbConn {
    match shared::db::establish_pool(name, default_max_connections).await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!(error = %e, pool = name, "failed to connect to database");
            std::process::exit(1);
        }
    }
}

fn connect_openbao() -> Option<openbao::OpenBao> {
    match openbao::OpenBao::from_environment() {
        Ok(client) => {
            tracing::info!("openbao client configured");
            Some(client)
        }
        Err(error) => {
            tracing::warn!(error = %error, "openbao client unavailable, vault endpoints disabled");
            None
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(exit_code) = recovery::dispatch(&args).await {
        std::process::exit(exit_code);
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    metrics::init();
    let log_receiver = telemetry::init_tracing();
    tracing::info!("api-gateway starting");

    if let Err(error) = api_gateway::auth::jwt::init_jwt_secret() {
        tracing::error!(error = %error, "invalid gateway configuration");
        std::process::exit(1);
    }

    let db_conn = connect_or_exit("", db::USER_DEFAULT_MAX_CONNECTIONS).await;
    let agent_db = connect_or_exit("agent", db::AGENT_DEFAULT_MAX_CONNECTIONS).await;
    let log_db = connect_or_exit("log", db::LOG_DEFAULT_MAX_CONNECTIONS).await;
    shared::spawn_log_writer(log_receiver, log_db);

    let default_org_id = match init::initialize_database(&db_conn).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize database");
            std::process::exit(1);
        }
    };

    let state = AppState {
        db_conn: db_conn.clone(),
        agent_db,
        service_client: service_client::ServiceClient::new(),
        default_org_id: Some(default_org_id),
        agent_stream_registry: routes::agent_stream::AgentStreamRegistry::new(),
        openbao: connect_openbao(),
        site_builder: site_client::SiteBuilder::from_environment(),
    };

    tracing::info!("starting log prune job");
    prune::spawn_log_prune_job(state.clone());

    vault_service::spawn_deletion_job(state.clone());
    vault_service::spawn_purge_job(state.clone());

    if let Err(error) = routes::sites::backfill_auto_domains(&state).await {
        tracing::warn!(error = %error, "auto domain backfill failed");
    }

    tracing::info!("starting ingress synchronization");
    ingress_sync::spawn(state.db_conn.clone());

    let app = routes::create_router()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state);

    let port = std::env::var("GATEWAY_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8000);
    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string());
    let addr: SocketAddr = format!("{}:{}", listen_addr, port).parse().unwrap();

    let tls_enabled =
        std::env::var("TLS_ENABLED").unwrap_or_else(|_| "true".to_string()) != "false";

    if tls_enabled {
        let config = tls::generate_tls_config()
            .await
            .expect("failed to generate TLS config");
        tracing::info!(version = env!("CARGO_PKG_VERSION"), addr = %addr, "listening (HTTPS)");
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    } else {
        tracing::info!(version = env!("CARGO_PKG_VERSION"), addr = %addr, "listening (HTTP)");
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    }
}
