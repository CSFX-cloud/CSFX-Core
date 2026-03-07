use sea_orm::DbConn;
use std::net::SocketAddr;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod auth;
mod auth_service;
mod db;
mod init;
mod metrics;
mod rbac_service;
mod routes;
mod self_monitor;
mod service_client;
mod system_collector;
mod telemetry;
mod utils;

use routes::registry::{
    AgentStatistics, ErrorResponse as RegistryErrorResponse, HeartbeatRequest,
    PreRegisterAgentRequest, PreRegisterAgentResponse, RegisterAgentRequest, RegisterAgentResponse,
};
use routes::users::{
    AuthResponse, LoginRequest, PublicKeyResponse, RegisterRequest, UserProfileResponse,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::users::register_user,
        routes::users::login_user,
        routes::users::logout_user,
        routes::users::get_public_key,
        routes::users::get_user_profile,
        routes::users::validate_session,
        routes::users::setup_2fa,
        routes::users::enable_2fa,
        routes::users::disable_2fa,
        routes::users::change_password,
        routes::users::change_email,
        routes::registry::pre_register_agent,
        routes::registry::list_pending_agents,
        routes::registry::delete_pending_agent,
        routes::registry::list_tokens,
        routes::registry::list_agents_admin,
        routes::registry::get_statistics,
        routes::registry::register_agent,
        routes::registry::agent_heartbeat,
        routes::registry::registry_health,
        routes::registry::create_token,
    ),
    components(
        schemas(
            RegisterRequest,
            LoginRequest,
            AuthResponse,
            PublicKeyResponse,
            UserProfileResponse,
            PreRegisterAgentRequest,
            PreRegisterAgentResponse,
            RegisterAgentRequest,
            RegisterAgentResponse,
            HeartbeatRequest,
            AgentStatistics,
            RegistryErrorResponse,
        )
    ),
    tags(
        (name = "Authentication", description = "User authentication and session management"),
        (name = "Encryption", description = "RSA public key for password encryption"),
        (name = "Registry - Admin", description = "Agent registry administration"),
        (name = "Registry - Agent", description = "Agent registration and heartbeat"),
        (name = "Registry - Health", description = "Registry service health check"),
        (name = "Registry - Admin (Deprecated)", description = "Deprecated token-based registration"),
        (name = "Agents", description = "Agent listing and metrics"),
        (name = "Volumes", description = "Volume lifecycle management (proxied to volume-manager)"),
        (name = "Workloads", description = "Workload scheduling (proxied to scheduler)"),
        (name = "Networks", description = "SDN network management (proxied to sdn-controller)"),
        (name = "Events", description = "Failover and system events (proxied to failover-controller)"),
        (name = "System", description = "Control plane system info and metrics"),
    ),
    modifiers(&SecurityAddon),
    info(
        title = "CSF Control Plane API",
        version = "0.2.0",
        description = "CS-Foundry Control Plane — agent registry, workload scheduling, volume management, SDN, failover, RBAC",
        contact(
            name = "CS-Foundry Team",
            email = "support@cs-foundry.com"
        )
    ),
    servers(
        (url = "http://localhost:8000/api", description = "Local development server")
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
            components.add_security_scheme(
                "api_key",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("X-API-Key"),
                    ),
                ),
            );
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db_conn: DbConn,
    pub service_client: service_client::ServiceClient,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    metrics::init();
    telemetry::init_tracing();

    let db_conn = match db::establish_connection().await {
        Ok(conn) => {
            tracing::info!("database connection established");
            conn
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to connect to database");
            std::process::exit(1);
        }
    };

    if let Err(e) = init::initialize_database(&db_conn).await {
        tracing::error!(error = %e, "failed to initialize database");
        std::process::exit(1);
    }

    let state = AppState {
        db_conn: db_conn.clone(),
        service_client: service_client::ServiceClient::new(),
    };

    tracing::info!("starting self-monitoring service");
    self_monitor::start_self_monitoring(std::sync::Arc::new(db_conn)).await;

    let app = routes::create_router()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!(addr = %addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
