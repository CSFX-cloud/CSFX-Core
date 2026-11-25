use shared::internal_auth::SignedRequest;
use std::collections::HashMap;

use crate::models::workload::BucketBinding;

const OBJECT_STORAGE_URL_ENV: &str = "OBJECT_STORAGE_URL";
const DEFAULT_OBJECT_STORAGE_URL: &str = "http://localhost:8006";

fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| shared::http::client(shared::http::DEFAULT_TIMEOUT))
}

fn base_url() -> String {
    std::env::var(OBJECT_STORAGE_URL_ENV).unwrap_or_else(|_| DEFAULT_OBJECT_STORAGE_URL.to_string())
}

pub async fn resolve_env_vars(
    workload_id: uuid::Uuid,
    resource_group_id: Option<uuid::Uuid>,
    bindings: &[BucketBinding],
) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let client = http_client();
    let base = base_url();

    for (index, binding) in bindings.iter().enumerate() {
        let bucket_id = binding.bucket_id;

        let bucket = match client
            .get(format!("{}/buckets/{}", base, bucket_id))
            .signed()
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(body) => body,
                    Err(e) => {
                        tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, error = %e, "failed to parse bucket response");
                        continue;
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, status = %resp.status(), "bucket lookup failed");
                continue;
            }
            Err(e) => {
                tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, error = %e, "failed to reach object-storage");
                continue;
            }
        };

        let global_alias = match bucket["global_alias"].as_str() {
            Some(alias) => alias.to_string(),
            None => {
                tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, "bucket response missing global_alias");
                continue;
            }
        };

        let key_name = format!("workload-{}", workload_id);
        let key_body = serde_json::json!({
            "name": key_name,
            "permissions": binding.permissions.clone().unwrap_or_else(|| "readwrite".to_string()),
        });

        let key = match client
            .post(format!("{}/buckets/{}/keys", base, bucket_id))
            .signed()
            .json(&key_body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(body) => body,
                    Err(e) => {
                        tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, error = %e, "failed to parse access key response");
                        continue;
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, status = %resp.status(), "access key creation failed");
                continue;
            }
            Err(e) => {
                tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, error = %e, "failed to reach object-storage");
                continue;
            }
        };

        let (Some(access_key_id), Some(secret_access_key)) = (
            key["garage_key_id"].as_str(),
            key["secret_access_key"].as_str(),
        ) else {
            tracing::warn!(workload_id = %workload_id, bucket_id = %bucket_id, "access key response missing credentials");
            continue;
        };

        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{}", index)
        };

        let endpoint = match resource_group_id {
            Some(rg_id) => format!("http://s3.svc.{}.internal:3900", rg_id),
            None => "http://127.0.0.1:3900".to_string(),
        };

        env.insert(
            format!("AWS_ACCESS_KEY_ID{}", suffix),
            access_key_id.to_string(),
        );
        env.insert(
            format!("AWS_SECRET_ACCESS_KEY{}", suffix),
            secret_access_key.to_string(),
        );
        env.insert(format!("AWS_BUCKET{}", suffix), global_alias);
        env.insert(format!("AWS_ENDPOINT_URL{}", suffix), endpoint);
    }

    env
}
