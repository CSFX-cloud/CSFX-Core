use entity::{entities::system_settings, SystemSettings};
use reqwest::Client;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const SETTINGS_KEY: &str = "notifications.ntfy";
const PUBLISH_TIMEOUT: Duration = Duration::from_secs(10);
const SEVERITIES: [&str; 3] = ["info", "warning", "critical"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtfyConfig {
    pub enabled: bool,
    pub server_url: String,
    pub topic: String,
    pub token: Option<String>,
    pub min_severity: String,
}

impl Default for NtfyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "https://ntfy.sh".to_string(),
            topic: String::new(),
            token: None,
            min_severity: "warning".to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NtfyError {
    #[error("invalid configuration: {0}")]
    Invalid(&'static str),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("ntfy request failed: {0}")]
    Request(#[from] reqwest::Error),
}

fn severity_rank(severity: &str) -> usize {
    SEVERITIES.iter().position(|s| *s == severity).unwrap_or(0)
}

fn priority(severity: &str) -> &'static str {
    match severity {
        "critical" => "5",
        "warning" => "4",
        _ => "3",
    }
}

impl NtfyConfig {
    pub fn validate(&self) -> Result<(), NtfyError> {
        if !self.server_url.starts_with("https://") && !self.server_url.starts_with("http://") {
            return Err(NtfyError::Invalid(
                "server_url must start with http:// or https://",
            ));
        }
        if self.topic.is_empty() || self.topic.contains(|c: char| c == '/' || c.is_whitespace()) {
            return Err(NtfyError::Invalid(
                "topic must be non-empty without slashes or spaces",
            ));
        }
        if !SEVERITIES.contains(&self.min_severity.as_str()) {
            return Err(NtfyError::Invalid(
                "min_severity must be info, warning or critical",
            ));
        }
        Ok(())
    }

    pub fn wants(&self, severity: &str) -> bool {
        self.enabled && severity_rank(severity) >= severity_rank(&self.min_severity)
    }

    pub async fn publish(
        &self,
        title: &str,
        message: &str,
        severity: &str,
    ) -> Result<(), NtfyError> {
        let url = format!("{}/{}", self.server_url.trim_end_matches('/'), self.topic);
        let mut request = Client::new()
            .post(url)
            .timeout(PUBLISH_TIMEOUT)
            .header("Title", title)
            .header("Priority", priority(severity))
            .body(message.to_string());
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        request.send().await?.error_for_status()?;
        Ok(())
    }
}

pub async fn load_config(db: &DatabaseConnection) -> Result<NtfyConfig, NtfyError> {
    let setting = SystemSettings::find_by_id(SETTINGS_KEY).one(db).await?;
    Ok(setting
        .and_then(|s| serde_json::from_value(s.value).ok())
        .unwrap_or_default())
}

pub async fn save_config(db: &DatabaseConnection, config: &NtfyConfig) -> Result<(), NtfyError> {
    let now = chrono::Utc::now().into();
    let value = json!(config);
    match SystemSettings::find_by_id(SETTINGS_KEY).one(db).await? {
        Some(setting) => {
            let mut active: system_settings::ActiveModel = setting.into();
            active.value = Set(value);
            active.updated_at = Set(now);
            active.update(db).await?;
        }
        None => {
            system_settings::ActiveModel {
                key: Set(SETTINGS_KEY.to_string()),
                value: Set(value),
                updated_at: Set(now),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

pub async fn notify_event(
    db: &DatabaseConnection,
    event_type: &str,
    severity: &str,
    message: Option<&str>,
) {
    let config = match load_config(db).await {
        Ok(config) if config.wants(severity) => config,
        Ok(_) => return,
        Err(e) => {
            tracing::error!(error = %e, "failed to load ntfy config");
            return;
        }
    };
    let title = event_type.replace('_', " ");
    if let Err(e) = config
        .publish(&title, message.unwrap_or(&title), severity)
        .await
    {
        tracing::warn!(error = %e, event_type, "failed to publish ntfy notification");
    }
}
