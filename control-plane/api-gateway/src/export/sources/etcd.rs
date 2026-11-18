use base64::{engine::general_purpose::STANDARD, Engine};
use etcd_client::{Client, GetOptions};
use serde_json::json;

use super::upstream;
use crate::export::{engine::Exporter, ExportError};

const DEFAULT_ENDPOINTS: &str = "http://localhost:2379";

pub async fn export(exporter: &mut Exporter) -> Result<(), ExportError> {
    let endpoints =
        std::env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| DEFAULT_ENDPOINTS.to_string());
    let mut client = Client::connect(endpoints.split(',').collect::<Vec<_>>(), None)
        .await
        .map_err(upstream)?;
    let response = client
        .get("", Some(GetOptions::new().with_all_keys()))
        .await
        .map_err(upstream)?;
    let mut lines = Vec::new();
    for pair in response.kvs() {
        let entry = json!({
            "key": STANDARD.encode(pair.key()),
            "value": STANDARD.encode(pair.value()),
        });
        lines.extend(serde_json::to_vec(&entry)?);
        lines.push(b'\n');
    }
    exporter
        .add_bytes("etcd/keys.jsonl".to_string(), "etcd", None, lines)
        .await
}
