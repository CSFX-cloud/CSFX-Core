use anyhow::{bail, Context, Result};
use axum::{
    body::Bytes,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
};
use reqwest::Client;
use shared::observability::{current_request_id, REQUEST_ID_HEADER};
use std::time::Duration;

const STREAM_READ_TIMEOUT: Duration = Duration::from_secs(300);

const FORWARDED_REQUEST_HEADERS: [header::HeaderName; 5] = [
    header::ACCEPT,
    header::ACCEPT_LANGUAGE,
    header::CONTENT_TYPE,
    header::HeaderName::from_static("x-api-key"),
    header::HeaderName::from_static("x-client-cert"),
];

#[derive(Clone, Copy, Debug)]
pub enum Service {
    Registry,
    Scheduler,
    VolumeManager,
    FailoverController,
    SdnController,
    ObjectStorage,
}

pub struct Upstream {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Upstream {
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

impl IntoResponse for Upstream {
    fn into_response(self) -> Response {
        (self.status, self.headers, self.body).into_response()
    }
}

fn request_headers(source: &HeaderMap, has_body: bool) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for name in FORWARDED_REQUEST_HEADERS {
        if let Some(value) = source.get(&name) {
            headers.insert(name, value.clone());
        }
    }
    if has_body && !headers.contains_key(header::CONTENT_TYPE) {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    if let Some(id) = current_request_id() {
        headers.insert(REQUEST_ID_HEADER.clone(), id);
    }
    headers.extend(shared::internal_auth::token_headers());
    headers
}

fn response_headers(source: &HeaderMap) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for name in [
        header::CONTENT_TYPE,
        header::HeaderName::from_static(shared::pagination::NEXT_CURSOR_HEADER),
    ] {
        if let Some(value) = source.get(&name) {
            headers.insert(name, value.clone());
        }
    }
    headers
}

fn reject_traversal(path: &str) -> Result<()> {
    let route = path.split_once('?').map_or(path, |(route, _)| route);
    if route.split('/').any(|segment| segment == "..") {
        bail!("path traversal rejected path={route}");
    }
    Ok(())
}

#[derive(Clone)]
pub struct ServiceClient {
    client: Client,
    stream_client: Client,
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

        let stream_client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(STREAM_READ_TIMEOUT)
            .build()
            .expect("Failed to create streaming HTTP client");

        Self {
            client,
            stream_client,
            registry_url,
            scheduler_url,
            volume_manager_url,
            failover_controller_url,
            sdn_controller_url,
            object_storage_url,
        }
    }

    fn base_url(&self, service: Service) -> &str {
        match service {
            Service::Registry => &self.registry_url,
            Service::Scheduler => &self.scheduler_url,
            Service::VolumeManager => &self.volume_manager_url,
            Service::FailoverController => &self.failover_controller_url,
            Service::SdnController => &self.sdn_controller_url,
            Service::ObjectStorage => &self.object_storage_url,
        }
    }

    async fn stream_to(
        &self,
        service: Service,
        method: Method,
        path: &str,
        body: Option<reqwest::Body>,
    ) -> Result<reqwest::Response> {
        reject_traversal(path)?;
        let url = format!("{}{path}", self.base_url(service));
        let mut request = self
            .stream_client
            .request(method, &url)
            .headers(request_headers(&HeaderMap::new(), false));
        if let Some(body) = body {
            request = request.body(body);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("streaming request to {url} failed"))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        Err(anyhow::anyhow!("{url} answered {status}: {detail}"))
    }

    pub async fn stream_object_storage(
        &self,
        method: Method,
        path: &str,
        body: Option<reqwest::Body>,
    ) -> Result<reqwest::Response> {
        self.stream_to(Service::ObjectStorage, method, path, body)
            .await
    }

    pub async fn stream_volume_manager(
        &self,
        method: Method,
        path: &str,
        body: Option<reqwest::Body>,
    ) -> Result<reqwest::Response> {
        self.stream_to(Service::VolumeManager, method, path, body)
            .await
    }

    pub async fn forward(
        &self,
        service: Service,
        method: Method,
        path: &str,
        headers: &HeaderMap,
        body: impl Into<Bytes>,
    ) -> Result<Upstream> {
        reject_traversal(path)?;
        let body = body.into();
        let url = format!("{}{path}", self.base_url(service));
        tracing::debug!(service = ?service, method = %method, url = %url, "forwarding request");
        let mut request = self
            .client
            .request(method, &url)
            .headers(request_headers(headers, !body.is_empty()));
        if !body.is_empty() {
            request = request.body(body);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("request to {url} failed"))?;
        let status = response.status();
        let headers = response_headers(response.headers());
        let body = response
            .bytes()
            .await
            .with_context(|| format!("reading response from {url} failed"))?;
        Ok(Upstream {
            status,
            headers,
            body,
        })
    }

    /// Health check for registry service
    pub async fn call_json(
        &self,
        service: Service,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let body = body.map_or_else(Bytes::new, |value| Bytes::from(value.to_string()));
        let upstream = self
            .forward(service, method, path, &HeaderMap::new(), body)
            .await?;
        let json = upstream.json();
        Ok((upstream.status, json))
    }

    pub async fn check_registry_health(&self) -> bool {
        let url = format!("{}/health", self.registry_url);
        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
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
                let upstream = client
                    .forward(
                        Service::Registry,
                        Method::GET,
                        "/echo",
                        request.headers(),
                        Bytes::new(),
                    )
                    .await
                    .unwrap();
                Json(upstream.json().unwrap())
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

        let upstream = client
            .forward(
                Service::Registry,
                Method::GET,
                "/echo",
                &HeaderMap::new(),
                Bytes::new(),
            )
            .await
            .unwrap();

        assert_eq!(upstream.json().unwrap()["ids"], json!([]));
    }

    async fn echo_text(headers: HeaderMap, body: String) -> (StatusCode, String) {
        let has_credentials =
            headers.contains_key("authorization") || headers.contains_key("cookie");
        (StatusCode::IM_A_TEAPOT, format!("{has_credentials}:{body}"))
    }

    #[tokio::test]
    async fn any_method_and_non_json_body_pass_through_without_credentials() {
        let router = Router::new().route("/echo", axum::routing::put(echo_text));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut client = client();
        client.registry_url = url;
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer secret".parse().unwrap());
        headers.insert("cookie", "session=1".parse().unwrap());

        let upstream = client
            .forward(
                Service::Registry,
                Method::PUT,
                "/echo",
                &headers,
                "plain text",
            )
            .await
            .unwrap();

        assert_eq!(upstream.status, StatusCode::IM_A_TEAPOT);
        assert_eq!(upstream.body, Bytes::from("false:plain text"));
    }

    #[tokio::test]
    async fn path_traversal_is_rejected() {
        let result = client()
            .forward(
                Service::Registry,
                Method::GET,
                "/admin/../secrets",
                &HeaderMap::new(),
                Bytes::new(),
            )
            .await;

        assert!(result.is_err());
    }
}
