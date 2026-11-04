use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(150);
const URL_VARIABLE: &str = "SITE_BUILDER_URL";
const SECRET_VARIABLE: &str = "CSFX_SITE_SECRET";
const DEFAULT_URL: &str = "http://localhost:8007";

#[derive(Debug, Error)]
pub enum SiteBuilderError {
    #[error("site builder is unreachable")]
    Unreachable,
    #[error("{0}")]
    Rejected(String),
}

#[derive(Clone)]
pub struct SiteBuilder {
    client: Client,
    base: String,
    secret: String,
}

impl SiteBuilder {
    pub fn from_environment() -> Option<Self> {
        let secret = std::env::var(SECRET_VARIABLE)
            .ok()
            .filter(|value| !value.is_empty())?;
        let base = std::env::var(URL_VARIABLE).unwrap_or_else(|_| DEFAULT_URL.to_string());
        let client = Client::builder().timeout(REQUEST_TIMEOUT).build().ok()?;
        Some(Self {
            client,
            base: base.trim_end_matches('/').to_string(),
            secret,
        })
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, SiteBuilderError> {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.secret)
            .json(body)
            .send()
            .await
            .map_err(|error| {
                tracing::warn!(path = %path, error = %error, "site builder request failed");
                SiteBuilderError::Unreachable
            })?;
        let status = response.status();
        let payload: Value = response.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            return Ok(payload);
        }
        let message = payload["error"].as_str().unwrap_or("request rejected");
        Err(SiteBuilderError::Rejected(message.to_string()))
    }
}
