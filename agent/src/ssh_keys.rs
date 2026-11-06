use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

use crate::client::build_http_client;
use crate::pki::AgentPki;

const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Deserialize)]
struct AuthorizedKeysResponse {
    keys: Vec<String>,
}

pub async fn fetch_authorized_keys(
    gateway_url: &str,
    agent_id: Uuid,
    username: &str,
) -> Result<Vec<String>> {
    let url = format!("{}/api/agents/{}/authorized-keys", gateway_url, agent_id);

    let ca_pem = AgentPki::load_ca_pem()?;
    let client = build_http_client(Some(&ca_pem))?;

    let resp = client
        .get(&url)
        .query(&[("user", username)])
        .timeout(REQUEST_TIMEOUT)
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

pub async fn run_authorized_keys_command(gateway_url: &str, agent_id: Uuid, username: &str) {
    match fetch_authorized_keys(gateway_url, agent_id, username).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_body_lists_keys() {
        let parsed: AuthorizedKeysResponse =
            serde_json::from_str(r#"{"keys":["ssh-ed25519 AAAA a","ssh-rsa BBBB b"]}"#).unwrap();
        assert_eq!(parsed.keys.len(), 2);
        assert!(serde_json::from_str::<AuthorizedKeysResponse>("{}").is_err());
    }
}
