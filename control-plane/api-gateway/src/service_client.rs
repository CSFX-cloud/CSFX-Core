use anyhow::{Context, Result};
use reqwest::{Client, RequestBuilder, StatusCode};
use shared::observability::{current_request_id, REQUEST_ID_HEADER};
use std::time::Duration;

#[derive(Clone, Copy)]
pub enum ListTarget {
    Scheduler,
    VolumeManager,
    FailoverController,
    ObjectStorage,
}

pub struct ListReply {
    pub status: StatusCode,
    pub body: Option<serde_json::Value>,
    pub next_cursor: Option<String>,
}

const SKIPPED_HEADERS: [&str; 5] = [
    "content-length",
    "host",
    "content-type",
    "transfer-encoding",
    "x-request-id",
];

fn with_forwarded_headers(
    mut request: RequestBuilder,
    headers: Option<Vec<(String, String)>>,
) -> RequestBuilder {
    for (key, value) in headers.into_iter().flatten() {
        if SKIPPED_HEADERS
            .iter()
            .any(|skipped| key.eq_ignore_ascii_case(skipped))
        {
            continue;
        }
        request = request.header(key, value);
    }
    match current_request_id() {
        Some(id) => request.header(REQUEST_ID_HEADER.clone(), id),
        None => request,
    }
}

/// HTTP Client for internal service-to-service communication
#[derive(Clone)]
pub struct ServiceClient {
    client: Client,
    registry_url: String,
    scheduler_url: String,
    volume_manager_url: String,
    failover_controller_url: String,
    sdn_controller_url: String,
    object_storage_url: String,
}

impl ServiceClient {
    pub fn new() -> Self {
        let registry_url = std::env::var("REGISTRY_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8001".to_string());

        let scheduler_url = std::env::var("SCHEDULER_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8002".to_string());

        let volume_manager_url = std::env::var("VOLUME_MANAGER_URL")
            .unwrap_or_else(|_| "http://localhost:8003".to_string());

        let failover_controller_url = std::env::var("FAILOVER_CONTROLLER_URL")
            .unwrap_or_else(|_| "http://localhost:8004".to_string());

        let sdn_controller_url = std::env::var("SDN_CONTROLLER_URL")
            .unwrap_or_else(|_| "http://localhost:8005".to_string());

        let object_storage_url = std::env::var("OBJECT_STORAGE_URL")
            .unwrap_or_else(|_| "http://localhost:8006".to_string());

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            registry_url,
            scheduler_url,
            volume_manager_url,
            failover_controller_url,
            sdn_controller_url,
            object_storage_url,
        }
    }

    pub async fn forward_list(
        &self,
        target: ListTarget,
        path_and_query: &str,
        headers: Vec<(String, String)>,
    ) -> Result<ListReply> {
        let base = match target {
            ListTarget::Scheduler => &self.scheduler_url,
            ListTarget::VolumeManager => &self.volume_manager_url,
            ListTarget::FailoverController => &self.failover_controller_url,
            ListTarget::ObjectStorage => &self.object_storage_url,
        };
        let request = with_forwarded_headers(
            self.client.get(format!("{}{}", base, path_and_query)),
            Some(headers),
        );
        let response = request
            .send()
            .await
            .context("Failed to send list request to service")?;
        let status = response.status();
        let next_cursor = response
            .headers()
            .get(shared::pagination::NEXT_CURSOR_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response
            .text()
            .await
            .ok()
            .filter(|text| !text.is_empty())
            .and_then(|text| serde_json::from_str(&text).ok());
        Ok(ListReply {
            status,
            body,
            next_cursor,
        })
    }

    /// Forward a request to the registry service
    pub async fn forward_to_registry(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let url = format!("{}{}", self.registry_url, path);

        tracing::debug!("Forwarding {} request to: {}", method, url);

        let mut request = match method {
            reqwest::Method::GET => self.client.get(&url),
            reqwest::Method::POST => self.client.post(&url),
            reqwest::Method::PUT => self.client.put(&url),
            reqwest::Method::DELETE => self.client.delete(&url),
            reqwest::Method::PATCH => self.client.patch(&url),
            _ => {
                return Err(anyhow::anyhow!("Unsupported HTTP method"));
            }
        };

        request = with_forwarded_headers(request, headers);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .context("Failed to send request to registry service")?;

        let status = response.status();
        let body_text = response.text().await.ok();

        let json_body = body_text.and_then(|text| {
            if text.is_empty() {
                None
            } else {
                serde_json::from_str(&text).ok()
            }
        });

        Ok((status, json_body))
    }

    /// Health check for registry service
    pub async fn check_registry_health(&self) -> bool {
        let url = format!("{}/health", self.registry_url);
        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    pub async fn forward_to_scheduler(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let url = format!("{}{}", self.scheduler_url, path);

        tracing::debug!("Forwarding {} request to scheduler: {}", method, url);

        let mut request = match method {
            reqwest::Method::GET => self.client.get(&url),
            reqwest::Method::POST => self.client.post(&url),
            reqwest::Method::DELETE => self.client.delete(&url),
            reqwest::Method::PATCH => self.client.patch(&url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method")),
        };

        request = with_forwarded_headers(request, headers);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .context("Failed to send request to scheduler service")?;

        let status = response.status();
        let body_text = response.text().await.ok();
        let json_body = body_text.and_then(|text| {
            if text.is_empty() {
                None
            } else {
                serde_json::from_str(&text).ok()
            }
        });

        Ok((status, json_body))
    }

    pub async fn forward_to_volume_manager(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let url = format!("{}{}", self.volume_manager_url, path);

        tracing::debug!("Forwarding {} request to volume-manager: {}", method, url);

        let mut request = match method {
            reqwest::Method::GET => self.client.get(&url),
            reqwest::Method::POST => self.client.post(&url),
            reqwest::Method::DELETE => self.client.delete(&url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method")),
        };

        request = with_forwarded_headers(request, headers);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .context("Failed to send request to volume-manager service")?;

        let status = response.status();
        let body_text = response.text().await.ok();
        let json_body = body_text.and_then(|text| {
            if text.is_empty() {
                None
            } else {
                serde_json::from_str(&text).ok()
            }
        });

        Ok((status, json_body))
    }

    pub async fn forward_to_failover_controller(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let url = format!("{}{}", self.failover_controller_url, path);

        tracing::debug!(
            "Forwarding {} request to failover-controller: {}",
            method,
            url
        );

        let mut request = match method {
            reqwest::Method::GET => self.client.get(&url),
            reqwest::Method::POST => self.client.post(&url),
            reqwest::Method::DELETE => self.client.delete(&url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method")),
        };

        request = with_forwarded_headers(request, headers);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .context("Failed to send request to failover-controller service")?;

        let status = response.status();
        let body_text = response.text().await.ok();
        let json_body = body_text.and_then(|text| {
            if text.is_empty() {
                None
            } else {
                serde_json::from_str(&text).ok()
            }
        });

        Ok((status, json_body))
    }

    pub async fn forward_to_sdn_controller(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let url = format!("{}{}", self.sdn_controller_url, path);

        tracing::debug!("Forwarding {} request to sdn-controller: {}", method, url);

        let mut request = match method {
            reqwest::Method::GET => self.client.get(&url),
            reqwest::Method::POST => self.client.post(&url),
            reqwest::Method::DELETE => self.client.delete(&url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method")),
        };

        request = with_forwarded_headers(request, headers);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .context("Failed to send request to sdn-controller service")?;

        let status = response.status();
        let body_text = response.text().await.ok();
        let json_body = body_text.and_then(|text| {
            if text.is_empty() {
                None
            } else {
                serde_json::from_str(&text).ok()
            }
        });

        Ok((status, json_body))
    }

    pub async fn forward_to_object_storage(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let url = format!("{}{}", self.object_storage_url, path);

        tracing::debug!("Forwarding {} request to object-storage: {}", method, url);

        let mut request = match method {
            reqwest::Method::GET => self.client.get(&url),
            reqwest::Method::POST => self.client.post(&url),
            reqwest::Method::PATCH => self.client.patch(&url),
            reqwest::Method::DELETE => self.client.delete(&url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method")),
        };

        request = with_forwarded_headers(request, headers);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .context("Failed to send request to object-storage service")?;

        let status = response.status();
        let body_text = response.text().await.ok();
        let json_body = body_text.and_then(|text| {
            if text.is_empty() {
                None
            } else {
                serde_json::from_str(&text).ok()
            }
        });

        Ok((status, json_body))
    }
}

impl Default for ServiceClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::HeaderMap;
    use axum::routing::get;
    use axum::{Json, Router};
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    async fn echo_request_ids(headers: HeaderMap) -> Json<Value> {
        let ids: Vec<&str> = headers
            .get_all("x-request-id")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .collect();
        Json(json!({ "ids": ids }))
    }

    fn client() -> ServiceClient {
        let _ = rustls::crypto::ring::default_provider().install_default();
        ServiceClient::new()
    }

    async fn spawn_mock_registry() -> String {
        let router = Router::new().route("/echo", get(echo_request_ids));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        url
    }

    async fn gateway(registry_url: String) -> Router {
        let mut client = client();
        client.registry_url = registry_url;
        let handler = move |request: Request| {
            let client = client.clone();
            async move {
                let headers = request
                    .headers()
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.to_string(),
                            value.to_str().unwrap_or_default().to_string(),
                        )
                    })
                    .collect();
                let (_, body) = client
                    .forward_to_registry(reqwest::Method::GET, "/echo", None, Some(headers))
                    .await
                    .unwrap();
                Json(body.unwrap())
            }
        };
        shared::observability::instrument(Router::new().route("/forward", get(handler)))
    }

    async fn call(app: &Router, request_id: Option<&str>) -> (String, Value) {
        let mut builder = Request::builder().uri("/forward");
        if let Some(id) = request_id {
            builder = builder.header("x-request-id", id);
        }
        let response = app
            .clone()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let echoed = response.headers()["x-request-id"]
            .to_str()
            .unwrap()
            .to_string();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (echoed, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn generated_request_id_reaches_the_downstream_service() {
        let app = gateway(spawn_mock_registry().await).await;

        let (echoed, body) = call(&app, None).await;

        assert!(uuid::Uuid::parse_str(&echoed).is_ok());
        assert_eq!(body["ids"], json!([echoed]));
    }

    #[tokio::test]
    async fn incoming_request_id_is_forwarded_once() {
        let app = gateway(spawn_mock_registry().await).await;

        let (echoed, body) = call(&app, Some("trace-me-42")).await;

        assert_eq!(echoed, "trace-me-42");
        assert_eq!(body["ids"], json!(["trace-me-42"]));
    }

    #[tokio::test]
    async fn no_request_id_is_sent_outside_a_request() {
        let mut client = client();
        client.registry_url = spawn_mock_registry().await;

        let (_, body) = client
            .forward_to_registry(reqwest::Method::GET, "/echo", None, None)
            .await
            .unwrap();

        assert_eq!(body.unwrap()["ids"], json!([]));
    }
}
