use sea_orm::DbConn;
use std::net::SocketAddr;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod auth;
mod auth_service;
mod db;
mod docker_service;
mod init;
mod metrics;
mod rbac_service;
mod routes;
mod self_monitor;
mod service_client;
mod system_collector;
mod utils;

use routes::expenses::{CreateExpenseRequest, UpdateExpenseRequest};
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
        // User & Authentication
        routes::users::register_user,
        routes::users::login_user,
        routes::users::logout_user,
        routes::users::get_public_key,
        routes::users::get_user_profile,
        // Expenses
        routes::expenses::get_expenses,
        routes::expenses::get_expense,
        routes::expenses::create_expense,
        // Registry - Admin
        routes::registry::pre_register_agent,
        routes::registry::list_pending_agents,
        routes::registry::delete_pending_agent,
        routes::registry::list_tokens,
        routes::registry::list_agents_admin,
        routes::registry::get_statistics,
        // Registry - Agent
        routes::registry::register_agent,
        routes::registry::agent_heartbeat,
        // Registry - Health
        routes::registry::registry_health,
        // Registry - Deprecated
        routes::registry::create_token,
    ),
    components(
        schemas(
            // User schemas
            RegisterRequest,
            LoginRequest,
            AuthResponse,
            PublicKeyResponse,
            UserProfileResponse,
            // Expense schemas
            CreateExpenseRequest,
            UpdateExpenseRequest,
            entity::expenses::Model,
            // Registry schemas
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
        (name = "Authentication", description = "User authentication endpoints"),
        (name = "Encryption", description = "RSA encryption endpoints"),
        (name = "Expenses", description = "Expense management endpoints"),
        (name = "Registry - Admin", description = "Registry administration endpoints (Pre-Registration workflow)"),
        (name = "Registry - Agent", description = "Agent registration and heartbeat endpoints"),
        (name = "Registry - Health", description = "Registry service health check"),
        (name = "Registry - Admin (Deprecated)", description = "Deprecated token-based registration"),
    ),
    modifiers(&SecurityAddon),
    info(
        title = "CSF Control Plane API",
        version = "0.1.0",
        description = "CS-Foundry Control Plane API with agent registry, Zero Trust security, and pre-registration workflow",
        contact(
            name = "CS-Foundry Team",
            email = "support@cs-foundry.com"
        )
    ),
    servers(
        (url = "http://localhost:8000", description = "Local development server")
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
    pub docker: Option<docker_service::DockerService>,
    pub service_client: service_client::ServiceClient,
}

impl Default for AppState {
    fn default() -> Self {
        // This is a placeholder implementation for middleware
        // In practice, you wouldn't use this default
        panic!("AppState should be created with actual database connection")
    }
}

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    metrics::init();

    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize database connection and run migrations
    let db_conn = match db::establish_connection().await {
        Ok(conn) => {
            tracing::info!("Database connection established successfully");
            conn
        }
        Err(e) => {
            tracing::error!("Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize database with default data (RSA keys, admin user)
    if let Err(e) = init::initialize_database(&db_conn).await {
        tracing::error!("Failed to initialize database: {}", e);
        std::process::exit(1);
    }

    // Initialize Docker service
    let docker = match docker_service::DockerService::new() {
        Ok(docker) => {
            if docker.is_available().await {
                tracing::info!("🐳 Docker service initialized successfully");
                Some(docker)
            } else {
                tracing::warn!("⚠️  Docker is installed but not running");
                None
            }
        }
        Err(e) => {
            tracing::warn!(
                "⚠️  Docker service not available: {}. Container management will be limited.",
                e
            );
            None
        }
    };

    // Create application state
    let state = AppState {
        db_conn: db_conn.clone(),
        docker,
        service_client: service_client::ServiceClient::new(),
    };

    // Start self-monitoring service
    tracing::info!("🔄 Starting self-monitoring service...");
    self_monitor::start_self_monitoring(std::sync::Arc::new(db_conn)).await;

    // build our application with a route
    let app = routes::create_router()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state);

    // run our app with hyper
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
