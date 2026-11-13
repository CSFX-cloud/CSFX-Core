use std::net::SocketAddr;
use std::sync::OnceLock;

use api_gateway::auth::crypto::{generate_salt, hash_password};
use api_gateway::auth::jwt::create_jwt;
use api_gateway::{init, routes, service_client, AppState};
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
use axum::routing::any;
use axum::{Json, Router};
use base64::Engine;
use http_body_util::BodyExt;
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use serde_json::{json, Value};
use tokio::sync::OnceCell;
use tower::ServiceExt;
use uuid::Uuid;

pub const ADMIN_PASSWORD: &str = "correct-horse-battery";

static MIGRATED: OnceCell<Uuid> = OnceCell::const_new();
static BACKEND: OnceLock<String> = OnceLock::new();

pub struct Harness {
    pub app: Router,
    pub db: DatabaseConnection,
    pub org_id: Uuid,
    pub admin_id: Uuid,
    pub admin_name: String,
    pub token: String,
}

async fn echo(request: Request<Body>) -> Json<Value> {
    let (parts, body) = request.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    Json(json!({
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "query": parts.uri.query(),
        "body": serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null),
    }))
}

fn backend_url() -> &'static str {
    BACKEND.get_or_init(|| {
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std_listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", std_listener.local_addr().unwrap());
        std::thread::spawn({
            let std_listener = std_listener;
            move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();
                    axum::serve(listener, Router::new().fallback(any(echo)))
                        .await
                        .unwrap();
                });
            }
        });
        for name in [
            "REGISTRY_SERVICE_URL",
            "SCHEDULER_SERVICE_URL",
            "VOLUME_MANAGER_URL",
            "FAILOVER_CONTROLLER_URL",
            "SDN_CONTROLLER_URL",
            "OBJECT_STORAGE_URL",
        ] {
            std::env::set_var(name, &url);
        }
        std::env::set_var("JWT_SECRET", "integration-test-secret-0123456789abcdef");
        url
    })
}

async fn connect() -> DatabaseConnection {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    Database::connect(&url)
        .await
        .expect("database must be reachable")
}

async fn prepare_database() -> Uuid {
    let db = connect().await;
    Migrator::up(&db, None)
        .await
        .expect("migrations must apply");
    init::initialize_database(&db)
        .await
        .expect("database must initialise")
}

async fn admin_role_id(db: &DatabaseConnection, org_id: Uuid) -> Uuid {
    entity::Role::find()
        .filter(entity::role::Column::Name.eq("Admin"))
        .filter(entity::role::Column::OrganizationId.eq(org_id))
        .one(db)
        .await
        .unwrap()
        .expect("admin role must exist")
        .id
}

pub async fn create_role(db: &DatabaseConnection, org_id: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    entity::role::ActiveModel {
        id: Set(id),
        name: Set(format!("role-{}", id.simple())),
        description: Set(None),
        organization_id: Set(org_id),
        is_system_role: Set(false),
        created_at: Set(chrono::Utc::now().naive_utc()),
    }
    .insert(db)
    .await
    .unwrap();
    id
}

async fn grant_permission(db: &DatabaseConnection, role_id: Uuid, name: &str) {
    let permission = entity::Permission::find()
        .filter(entity::permission::Column::Name.eq(name))
        .one(db)
        .await
        .unwrap()
        .expect("permission must be seeded");
    entity::role_permission::ActiveModel {
        role_id: Set(role_id),
        permission_id: Set(permission.id),
    }
    .insert(db)
    .await
    .unwrap();
}

pub async fn create_user(db: &DatabaseConnection, org_id: Uuid, role_id: Uuid) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let name = format!("user-{}", id.simple());
    let salt = generate_salt();
    entity::user::ActiveModel {
        id: Set(id),
        name: Set(name.clone()),
        password: Set(hash_password(ADMIN_PASSWORD, &salt).await.unwrap()),
        salt: Set(salt),
        email: Set(Some(format!("{}@csfx.test", name))),
        gravatar_email: Set(None),
        two_factor_secret: Set(None),
        two_factor_enabled: Set(false),
        force_password_change: Set(false),
        auth_source: Set("local".to_string()),
    }
    .insert(db)
    .await
    .unwrap();
    entity::user_organization::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(id),
        organization_id: Set(org_id),
        role_id: Set(role_id),
        joined_at: Set(chrono::Utc::now().naive_utc()),
    }
    .insert(db)
    .await
    .unwrap();
    (id, name)
}

pub async fn harness() -> Harness {
    backend_url();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let org_id = *MIGRATED.get_or_init(prepare_database).await;
    let db = connect().await;
    let role_id = admin_role_id(&db, org_id).await;
    let (admin_id, admin_name) = create_user(&db, org_id, role_id).await;
    let token = create_jwt(admin_id, admin_name.clone()).unwrap();
    let state = AppState {
        db_conn: db.clone(),
        agent_db: db.clone(),
        service_client: service_client::ServiceClient::new(),
        default_org_id: Some(org_id),
        agent_stream_registry: routes::agent_stream::AgentStreamRegistry::new(),
        openbao: None,
        site_builder: None,
    };
    let app = routes::create_router(state.clone()).with_state(state);
    Harness {
        app,
        db,
        org_id,
        admin_id,
        admin_name,
        token,
    }
}

impl Harness {
    pub async fn send(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        self.send_from("127.0.0.1:4000", method, uri, token, body)
            .await
    }

    pub async fn send_from(
        &self,
        peer: &str,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {}", token));
        }
        let payload = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
        let mut request = builder.body(payload).unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    pub async fn admin(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        self.send(method, uri, Some(&self.token), body).await
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.admin(Method::GET, uri, None).await
    }

    pub async fn post(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.admin(Method::POST, uri, Some(body)).await
    }
}

pub fn encrypt_password(public_key_pem: &str, password: &str) -> String {
    use rsa::pkcs1::DecodeRsaPublicKey;
    use rsa::rand_core::OsRng;
    use rsa::sha2::Sha256;
    use rsa::{Oaep, RsaPublicKey};

    let key = RsaPublicKey::from_pkcs1_pem(public_key_pem).unwrap();
    let encrypted = key
        .encrypt(&mut OsRng, Oaep::new::<Sha256>(), password.as_bytes())
        .unwrap();
    base64::engine::general_purpose::STANDARD.encode(encrypted)
}

impl Harness {
    pub async fn limited_token(&self) -> String {
        let role_id = create_role(&self.db, self.org_id).await;
        let (id, name) = create_user(&self.db, self.org_id, role_id).await;
        create_jwt(id, name).unwrap()
    }

    pub async fn viewer_token(&self) -> String {
        let role_id = create_role(&self.db, self.org_id).await;
        grant_permission(&self.db, role_id, "agents.view").await;
        let (id, name) = create_user(&self.db, self.org_id, role_id).await;
        create_jwt(id, name).unwrap()
    }

    pub async fn token_with_permissions(&self, names: &[&str]) -> (Uuid, String) {
        let role_id = create_role(&self.db, self.org_id).await;
        for name in names {
            grant_permission(&self.db, role_id, name).await;
        }
        let (id, name) = create_user(&self.db, self.org_id, role_id).await;
        (id, create_jwt(id, name).unwrap())
    }

    pub async fn admin_role(&self) -> Uuid {
        admin_role_id(&self.db, self.org_id).await
    }

    pub async fn role_with_permissions(&self, names: &[&str]) -> Uuid {
        let role_id = create_role(&self.db, self.org_id).await;
        for name in names {
            grant_permission(&self.db, role_id, name).await;
        }
        role_id
    }

    pub async fn permission_id(&self, name: &str) -> Uuid {
        entity::Permission::find()
            .filter(entity::permission::Column::Name.eq(name))
            .one(&self.db)
            .await
            .unwrap()
            .expect("permission must be seeded")
            .id
    }

    pub async fn encrypted(&self, password: &str) -> String {
        let (_, body) = self.send(Method::GET, "/api/public-key", None, None).await;
        encrypt_password(body["public_key"].as_str().unwrap(), password)
    }

    pub async fn user_with_token(&self) -> (Uuid, String, String) {
        let role_id = admin_role_id(&self.db, self.org_id).await;
        let (id, name) = create_user(&self.db, self.org_id, role_id).await;
        let token = create_jwt(id, name.clone()).unwrap();
        (id, name, token)
    }
}
