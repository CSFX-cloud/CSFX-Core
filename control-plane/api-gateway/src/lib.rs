use sea_orm::DbConn;
use utoipa::OpenApi;

pub mod admins;
pub mod audit;
pub mod auth;
pub mod auth_service;
pub mod cef;
pub mod cluster_bundle;
pub mod console_pin;
pub mod db;
pub mod dns_sync;
pub mod etcd_members;
pub mod hostname_policy;
pub mod ingress_sync;
pub mod init;
pub mod metrics;
pub mod oidc;
pub mod openbao;
pub mod prune;
pub mod rbac_service;
pub mod recovery;
pub mod role_grants;
pub mod routes;
pub mod service_client;
pub mod setup;
pub mod site_client;
pub mod site_service;
pub mod system_collector;
pub mod telemetry;
pub mod tls;
pub mod utils;
pub mod vault_service;
pub mod vault_store;
pub mod zone_cert;

use routes::registry::{
    AgentStatistics, ErrorResponse as RegistryErrorResponse, HeartbeatRequest,
    PreRegisterAgentRequest, PreRegisterAgentResponse, RegisterAgentRequest, RegisterAgentResponse,
};
use routes::users::{AuthResponse, LoginRequest, PublicKeyResponse, UserProfileResponse};

#[derive(OpenApi)]
#[openapi(
    paths(
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
        title = "CSFX Control Plane API",
        version = "0.2.0",
        description = "CSFX Control Plane — agent registry, workload scheduling, volume management, SDN, failover, RBAC",
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
    pub agent_db: DbConn,
    pub service_client: service_client::ServiceClient,
    pub default_org_id: Option<uuid::Uuid>,
    pub agent_stream_registry: routes::agent_stream::AgentStreamRegistry,
    pub openbao: Option<openbao::OpenBao>,
    pub site_builder: Option<site_client::SiteBuilder>,
}
