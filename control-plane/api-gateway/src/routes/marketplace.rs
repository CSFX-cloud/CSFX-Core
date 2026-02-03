use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use entity::entities::marketplace_templates;
use entity::MarketplaceTemplates;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthenticatedUser;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketplaceTemplateResponse {
    pub id: Uuid,
    pub template_id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    pub resource_type: String,
    pub configuration: serde_json::Value,
    pub popular: bool,
    pub install_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallTemplateRequest {
    pub template_id: String,
    pub name: String,
    pub resource_group_id: Uuid,
}

impl From<marketplace_templates::Model> for MarketplaceTemplateResponse {
    fn from(model: marketplace_templates::Model) -> Self {
        Self {
            id: model.id,
            template_id: model.template_id,
            name: model.name,
            description: model.description,
            icon: model.icon,
            category: model.category,
            resource_type: model.resource_type,
            configuration: model.configuration,
            popular: model.popular,
            install_count: model.install_count,
        }
    }
}

/// List all marketplace templates
async fn list_templates(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> impl IntoResponse {
    let db = &state.db_conn;

    match MarketplaceTemplates::find()
        .order_by_desc(marketplace_templates::Column::Popular)
        .order_by_desc(marketplace_templates::Column::InstallCount)
        .all(db)
        .await
    {
        Ok(templates) => {
            let response: Vec<MarketplaceTemplateResponse> =
                templates.into_iter().map(|t| t.into()).collect();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get marketplace templates: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve marketplace templates"
                })),
            )
                .into_response()
        }
    }
}

/// Get a specific template by ID
async fn get_template(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(template_id): Path<String>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    match MarketplaceTemplates::find()
        .filter(marketplace_templates::Column::TemplateId.eq(template_id))
        .one(db)
        .await
    {
        Ok(Some(template)) => {
            let response: MarketplaceTemplateResponse = template.into();
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Template not found"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get template: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve template"
                })),
            )
                .into_response()
        }
    }
}

/// Get popular templates
async fn list_popular_templates(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> impl IntoResponse {
    let db = &state.db_conn;

    match MarketplaceTemplates::find()
        .filter(marketplace_templates::Column::Popular.eq(true))
        .order_by_desc(marketplace_templates::Column::InstallCount)
        .all(db)
        .await
    {
        Ok(templates) => {
            let response: Vec<MarketplaceTemplateResponse> =
                templates.into_iter().map(|t| t.into()).collect();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get popular templates: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve popular templates"
                })),
            )
                .into_response()
        }
    }
}

/// Install a template (creates a new resource from template)
async fn install_template(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<InstallTemplateRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the template
    let template = match MarketplaceTemplates::find()
        .filter(marketplace_templates::Column::TemplateId.eq(&payload.template_id))
        .one(db)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Template not found"
                })),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get template: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve template"
                })),
            )
                .into_response();
        }
    };

    // Create a new resource from the template
    use entity::entities::docker_resources;
    let now = chrono::Utc::now().naive_utc();
    let new_resource = docker_resources::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        name: ActiveValue::Set(payload.name.clone()),
        resource_type: ActiveValue::Set(template.resource_type.clone()),
        description: ActiveValue::Set(Some(template.description.clone())),
        resource_group_id: ActiveValue::Set(payload.resource_group_id),
        configuration: ActiveValue::Set(Some(template.configuration.clone())),
        status: ActiveValue::Set("pending".to_string()),
        created_by: ActiveValue::Set(Some(user.0.user_id)),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
        tags: ActiveValue::Set(Some(serde_json::json!({
            "template_id": payload.template_id,
            "source": "marketplace"
        }))),
        container_id: ActiveValue::NotSet,
        stack_name: ActiveValue::NotSet,
    };

    match new_resource.insert(db).await {
        Ok(resource) => {
            // Increment install count
            let mut template_model: marketplace_templates::ActiveModel = template.into();
            if let ActiveValue::Set(count) = template_model.install_count {
                template_model.install_count = ActiveValue::Set(count + 1);
            } else {
                template_model.install_count = ActiveValue::Set(1);
            }
            template_model.updated_at = ActiveValue::Set(now);

            let _ = template_model.update(db).await;

            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": resource.id,
                    "name": resource.name,
                    "resource_type": resource.resource_type,
                    "status": resource.status,
                    "message": "Template installed successfully"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to install template: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to install template"
                })),
            )
                .into_response()
        }
    }
}

/// Seed marketplace with default templates (admin only)
async fn seed_marketplace(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> impl IntoResponse {
    let db = &state.db_conn;

    let default_templates = vec![
        // Base Templates
        (
            "docker-container",
            "Docker Container",
            "Leerer Docker Container. Wähle dein eigenes Image und konfiguriere es nach deinen Wünschen.",
            "🐳",
            "base",
            "docker-container",
            serde_json::json!({
                "image": "nginx:alpine",
                "ports": [{"container": 80, "host": 8080}],
                "environment": {},
                "volumes": [],
                "restart_policy": "unless-stopped"
            }),
            false,
        ),
        (
            "docker-stack",
            "Docker Stack",
            "Leerer Docker Stack. Erstelle einen Multi-Container Stack mit mehreren Services.",
            "📦",
            "base",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "app",
                        "image": "nginx:alpine",
                        "ports": [{"container": 80, "host": 8080}],
                        "environment": {},
                        "volumes": [],
                        "restart_policy": "unless-stopped"
                    }
                ]
            }),
            false,
        ),
        // Popular Stacks
        (
            "wordpress",
            "WordPress + MySQL",
            "Vollständiger WordPress Stack mit MySQL Datenbank. Perfekt für Blogs, Websites und Content Management.",
            "📝",
            "cms",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "wordpress",
                        "image": "wordpress:latest",
                        "ports": [{"container": 80, "host": 8080}],
                        "environment": {
                            "WORDPRESS_DB_HOST": "db:3306",
                            "WORDPRESS_DB_USER": "wordpress",
                            "WORDPRESS_DB_PASSWORD": "wordpress",
                            "WORDPRESS_DB_NAME": "wordpress"
                        },
                        "depends_on": ["db"],
                        "restart_policy": "always"
                    },
                    {
                        "name": "db",
                        "image": "mysql:8.0",
                        "environment": {
                            "MYSQL_DATABASE": "wordpress",
                            "MYSQL_USER": "wordpress",
                            "MYSQL_PASSWORD": "wordpress",
                            "MYSQL_RANDOM_ROOT_PASSWORD": "1"
                        },
                        "volumes": [{"host": "./db_data", "container": "/var/lib/mysql"}],
                        "restart_policy": "always"
                    }
                ]
            }),
            true,
        ),
        (
            "mern-stack",
            "MERN Stack",
            "MongoDB + Express + React + Node.js Entwicklungsumgebung. Komplett JavaScript Stack.",
            "🚀",
            "development",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "mongodb",
                        "image": "mongo:7",
                        "ports": [{"container": 27017, "host": 27017}],
                        "volumes": [{"host": "./mongodb_data", "container": "/data/db"}],
                        "restart_policy": "always"
                    },
                    {
                        "name": "backend",
                        "image": "node:20-alpine",
                        "ports": [{"container": 5000, "host": 5000}],
                        "environment": {
                            "MONGODB_URI": "mongodb://mongodb:27017/app",
                            "NODE_ENV": "development"
                        },
                        "depends_on": ["mongodb"],
                        "restart_policy": "always"
                    },
                    {
                        "name": "frontend",
                        "image": "node:20-alpine",
                        "ports": [{"container": 3000, "host": 3000}],
                        "environment": {
                            "REACT_APP_API_URL": "http://localhost:5000"
                        },
                        "restart_policy": "always"
                    }
                ]
            }),
            true,
        ),
        (
            "nginx-postgres",
            "NGINX + PostgreSQL",
            "Web Server mit PostgreSQL Datenbank. Ideal für Web-Anwendungen.",
            "🌐",
            "web",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "nginx",
                        "image": "nginx:alpine",
                        "ports": [{"container": 80, "host": 8080}],
                        "volumes": [{"host": "./html", "container": "/usr/share/nginx/html"}],
                        "depends_on": ["postgres"],
                        "restart_policy": "always"
                    },
                    {
                        "name": "postgres",
                        "image": "postgres:16",
                        "environment": {
                            "POSTGRES_USER": "admin",
                            "POSTGRES_PASSWORD": "admin",
                            "POSTGRES_DB": "appdb"
                        },
                        "volumes": [{"host": "./postgres_data", "container": "/var/lib/postgresql/data"}],
                        "restart_policy": "always"
                    }
                ]
            }),
            false,
        ),
        // Databases
        (
            "postgresql",
            "PostgreSQL Database",
            "Leistungsstarke open-source relationale Datenbank. Perfekt für Production-Workloads.",
            "🐘",
            "database",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "postgres",
                        "image": "postgres:16-alpine",
                        "ports": [{"container": 5432, "host": 5432}],
                        "environment": {
                            "POSTGRES_USER": "admin",
                            "POSTGRES_PASSWORD": "changeme",
                            "POSTGRES_DB": "maindb"
                        },
                        "volumes": [{"host": "./postgres_data", "container": "/var/lib/postgresql/data"}],
                        "restart_policy": "always"
                    }
                ]
            }),
            true,
        ),
        (
            "mysql",
            "MySQL Database",
            "Beliebte open-source relationale Datenbank. Weit verbreitet und zuverlässig.",
            "🔷",
            "database",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "mysql",
                        "image": "mysql:8.0",
                        "ports": [{"container": 3306, "host": 3306}],
                        "environment": {
                            "MYSQL_ROOT_PASSWORD": "rootpass",
                            "MYSQL_DATABASE": "appdb",
                            "MYSQL_USER": "appuser",
                            "MYSQL_PASSWORD": "apppass"
                        },
                        "volumes": [{"host": "./mysql_data", "container": "/var/lib/mysql"}],
                        "restart_policy": "always"
                    }
                ]
            }),
            true,
        ),
        (
            "mongodb",
            "MongoDB Database",
            "NoSQL Datenbank für moderne Anwendungen. Flexibles Schema und horizontal skalierbar.",
            "🍃",
            "database",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "mongodb",
                        "image": "mongo:7",
                        "ports": [{"container": 27017, "host": 27017}],
                        "environment": {
                            "MONGO_INITDB_ROOT_USERNAME": "admin",
                            "MONGO_INITDB_ROOT_PASSWORD": "changeme"
                        },
                        "volumes": [{"host": "./mongodb_data", "container": "/data/db"}],
                        "restart_policy": "always"
                    }
                ]
            }),
            true,
        ),
        (
            "redis",
            "Redis Cache",
            "In-Memory Datenstruktur-Speicher. Perfekt als Cache, Message Broker oder Session Store.",
            "⚡",
            "cache",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "redis",
                        "image": "redis:7-alpine",
                        "ports": [{"container": 6379, "host": 6379}],
                        "volumes": [{"host": "./redis_data", "container": "/data"}],
                        "restart_policy": "always"
                    }
                ]
            }),
            true,
        ),
        // Web Servers & Proxies
        (
            "nginx",
            "NGINX Web Server",
            "Hochperformanter Web Server und Reverse Proxy. Ideal für Static Files und Load Balancing.",
            "🌐",
            "web",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "nginx",
                        "image": "nginx:alpine",
                        "ports": [{"container": 80, "host": 8080}],
                        "volumes": [
                            {"host": "./html", "container": "/usr/share/nginx/html"},
                            {"host": "./nginx.conf", "container": "/etc/nginx/nginx.conf"}
                        ],
                        "restart_policy": "always"
                    }
                ]
            }),
            false,
        ),
        (
            "traefik",
            "Traefik Reverse Proxy",
            "Moderner HTTP Reverse Proxy und Load Balancer. Automatische SSL und Service Discovery.",
            "🔀",
            "proxy",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "traefik",
                        "image": "traefik:v2.10",
                        "ports": [
                            {"container": 80, "host": 80},
                            {"container": 443, "host": 443},
                            {"container": 8080, "host": 8080}
                        ],
                        "volumes": [
                            {"host": "/var/run/docker.sock", "container": "/var/run/docker.sock"},
                            {"host": "./traefik.yml", "container": "/etc/traefik/traefik.yml"}
                        ],
                        "restart_policy": "always"
                    }
                ]
            }),
            false,
        ),
        // Development Tools
        (
            "gitlab",
            "GitLab CE",
            "Komplette DevOps Platform. Git Repository, CI/CD, Issue Tracking und mehr.",
            "🦊",
            "devops",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "gitlab",
                        "image": "gitlab/gitlab-ce:latest",
                        "ports": [
                            {"container": 80, "host": 8929},
                            {"container": 443, "host": 8943},
                            {"container": 22, "host": 2222}
                        ],
                        "volumes": [
                            {"host": "./gitlab/config", "container": "/etc/gitlab"},
                            {"host": "./gitlab/logs", "container": "/var/log/gitlab"},
                            {"host": "./gitlab/data", "container": "/var/opt/gitlab"}
                        ],
                        "restart_policy": "always"
                    }
                ]
            }),
            false,
        ),
        (
            "portainer",
            "Portainer",
            "Docker Management UI. Verwalte Container, Images, Volumes und Networks grafisch.",
            "🐳",
            "management",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "portainer",
                        "image": "portainer/portainer-ce:latest",
                        "ports": [
                            {"container": 9000, "host": 9000},
                            {"container": 8000, "host": 8000}
                        ],
                        "volumes": [
                            {"host": "/var/run/docker.sock", "container": "/var/run/docker.sock"},
                            {"host": "./portainer_data", "container": "/data"}
                        ],
                        "restart_policy": "always"
                    }
                ]
            }),
            false,
        ),
        // Monitoring & Analytics
        (
            "grafana-prometheus",
            "Grafana + Prometheus",
            "Monitoring Stack. Metriken sammeln mit Prometheus und visualisieren mit Grafana.",
            "📊",
            "monitoring",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "prometheus",
                        "image": "prom/prometheus:latest",
                        "ports": [{"container": 9090, "host": 9090}],
                        "volumes": [
                            {"host": "./prometheus.yml", "container": "/etc/prometheus/prometheus.yml"},
                            {"host": "./prometheus_data", "container": "/prometheus"}
                        ],
                        "restart_policy": "always"
                    },
                    {
                        "name": "grafana",
                        "image": "grafana/grafana:latest",
                        "ports": [{"container": 3000, "host": 3000}],
                        "environment": {
                            "GF_SECURITY_ADMIN_PASSWORD": "admin"
                        },
                        "volumes": [{"host": "./grafana_data", "container": "/var/lib/grafana"}],
                        "depends_on": ["prometheus"],
                        "restart_policy": "always"
                    }
                ]
            }),
            true,
        ),
        (
            "elk-stack",
            "ELK Stack",
            "Elasticsearch + Logstash + Kibana. Zentrale Log-Verwaltung und Analyse.",
            "📈",
            "logging",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "elasticsearch",
                        "image": "elasticsearch:8.11.0",
                        "ports": [{"container": 9200, "host": 9200}],
                        "environment": {
                            "discovery.type": "single-node",
                            "xpack.security.enabled": "false"
                        },
                        "volumes": [{"host": "./elasticsearch_data", "container": "/usr/share/elasticsearch/data"}],
                        "restart_policy": "always"
                    },
                    {
                        "name": "kibana",
                        "image": "kibana:8.11.0",
                        "ports": [{"container": 5601, "host": 5601}],
                        "environment": {
                            "ELASTICSEARCH_HOSTS": "http://elasticsearch:9200"
                        },
                        "depends_on": ["elasticsearch"],
                        "restart_policy": "always"
                    }
                ]
            }),
            false,
        ),
    ];

    let now = chrono::Utc::now().naive_utc();
    let mut created_count = 0;

    for (template_id, name, description, icon, category, resource_type, configuration, popular) in
        default_templates
    {
        // Check if template already exists
        let exists = MarketplaceTemplates::find()
            .filter(marketplace_templates::Column::TemplateId.eq(template_id))
            .one(db)
            .await
            .unwrap_or(None)
            .is_some();

        if !exists {
            let template = marketplace_templates::ActiveModel {
                id: ActiveValue::Set(Uuid::new_v4()),
                template_id: ActiveValue::Set(template_id.to_string()),
                name: ActiveValue::Set(name.to_string()),
                description: ActiveValue::Set(description.to_string()),
                icon: ActiveValue::Set(icon.to_string()),
                category: ActiveValue::Set(category.to_string()),
                resource_type: ActiveValue::Set(resource_type.to_string()),
                configuration: ActiveValue::Set(configuration),
                popular: ActiveValue::Set(popular),
                install_count: ActiveValue::Set(0),
                created_at: ActiveValue::Set(now),
                updated_at: ActiveValue::Set(now),
            };

            if template.insert(db).await.is_ok() {
                created_count += 1;
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "message": format!("Seeded {} templates", created_count),
            "created": created_count
        })),
    )
        .into_response()
}

pub fn marketplace_routes() -> Router<AppState> {
    Router::new()
        .route("/marketplace/templates", get(list_templates))
        .route(
            "/marketplace/templates/popular",
            get(list_popular_templates),
        )
        .route("/marketplace/templates/:template_id", get(get_template))
        .route("/marketplace/install", post(install_template))
        .route("/marketplace/seed", post(seed_marketplace))
}
