use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// HTTP Client for internal service-to-service communication
#[derive(Clone)]
pub struct ServiceClient {
    client: Client,
    registry_url: String,
    scheduler_url: String,
    volume_manager_url: String,
    failover_controller_url: String,
    sdn_controller_url: String,
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
        }
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

        if let Some(headers) = headers {
            for (key, value) in headers {
                let key_lower = key.to_lowercase();
                if key_lower == "content-length"
                    || key_lower == "host"
                    || key_lower == "content-type"
                    || key_lower == "transfer-encoding"
                {
                    continue;
                }
                request = request.header(key, value);
            }
        }

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
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method")),
        };

        if let Some(headers) = headers {
            for (key, value) in headers {
                let key_lower = key.to_lowercase();
                if key_lower == "content-length"
                    || key_lower == "host"
                    || key_lower == "content-type"
                    || key_lower == "transfer-encoding"
                {
                    continue;
                }
                request = request.header(key, value);
            }
        }

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

        if let Some(headers) = headers {
            for (key, value) in headers {
                let key_lower = key.to_lowercase();
                if key_lower == "content-length"
                    || key_lower == "host"
                    || key_lower == "content-type"
                    || key_lower == "transfer-encoding"
                {
                    continue;
                }
                request = request.header(key, value);
            }
        }

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

        tracing::debug!("Forwarding {} request to failover-controller: {}", method, url);

        let mut request = match method {
            reqwest::Method::GET => self.client.get(&url),
            reqwest::Method::POST => self.client.post(&url),
            reqwest::Method::DELETE => self.client.delete(&url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method")),
        };

        if let Some(headers) = headers {
            for (key, value) in headers {
                let key_lower = key.to_lowercase();
                if key_lower == "content-length"
                    || key_lower == "host"
                    || key_lower == "content-type"
                    || key_lower == "transfer-encoding"
                {
                    continue;
                }
                request = request.header(key, value);
            }
        }

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

        if let Some(headers) = headers {
            for (key, value) in headers {
                let key_lower = key.to_lowercase();
                if key_lower == "content-length"
                    || key_lower == "host"
                    || key_lower == "content-type"
                    || key_lower == "transfer-encoding"
                {
                    continue;
                }
                request = request.header(key, value);
            }
        }

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
}

// Response types for common registry operations
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTokenRequest {
    pub description: Option<String>,
    pub created_by: String,
    pub ttl_hours: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTokenResponse {
    pub token_id: String,
    pub token: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateTokenRequest {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateTokenResponse {
    pub valid: bool,
    pub agent_id: Option<String>,
}
