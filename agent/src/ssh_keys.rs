use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct AuthorizedKeysResponse {
    keys: Vec<String>,
}

pub async fn fetch_authorized_keys(gateway_url: &str, agent_id: Uuid) -> Result<Vec<String>> {
    let url = format!(
        "{}/api/agents/{}/authorized-keys",
        gateway_url, agent_id
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .context("failed to build http client")?;

    let resp = client
        .get(&url)
        .send()
        .await
        .context("failed to fetch authorized keys")?;

    if !resp.status().is_success() {
        anyhow::bail!("authorized keys fetch failed status={}", resp.status());
    }

    let body: AuthorizedKeysResponse = resp
        .json()
        .await
        .context("failed to parse authorized keys response")?;

    Ok(body.keys)
}

pub async fn run_authorized_keys_command(gateway_url: &str, agent_id: Uuid) {
    match fetch_authorized_keys(gateway_url, agent_id).await {
        Ok(keys) => {
            for key in keys {
                println!("{}", key);
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch authorized keys");
        }
    }
}
