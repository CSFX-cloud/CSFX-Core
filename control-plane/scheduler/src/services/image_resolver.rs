use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

const API_GATEWAY_URL_ENV: &str = "API_GATEWAY_URL";
const DEFAULT_API_GATEWAY_URL: &str = "http://localhost:8000";
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(RESOLVE_TIMEOUT)
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_default()
    })
}

async fn request_resolution(resource_group_id: Uuid, image: &str) -> Result<String, String> {
    let base_url =
        std::env::var(API_GATEWAY_URL_ENV).unwrap_or_else(|_| DEFAULT_API_GATEWAY_URL.to_string());
    let url = format!(
        "{}/api/internal/resource-groups/{}/resolve-image",
        base_url, resource_group_id
    );

    let response = http_client()
        .post(&url)
        .json(&serde_json::json!({ "image": image }))
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| e.to_string())?;

    let body: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    body["image"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "resolution response has no image".to_string())
}

pub async fn resolve(resource_group_id: Option<Uuid>, image: &str) -> String {
    let Some(resource_group_id) = resource_group_id else {
        return image.to_string();
    };

    match request_resolution(resource_group_id, image).await {
        Ok(resolved) => resolved,
        Err(e) => {
            crate::log_warn!(
                "image_resolver",
                &format!(
                    "Image resolution failed, using reference as written resource_group_id={} image={} err={}",
                    resource_group_id, image, e
                )
            );
            image.to_string()
        }
    }
}
