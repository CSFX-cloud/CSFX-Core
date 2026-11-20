use axum::http::HeaderMap;
use entity::entities::workloads;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::Method;
use shared::images::bucket_image_reference;

use crate::{routes::buckets::presign_path, service_client::Service, AppState};

const KEY_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC.remove(b'/');

pub async fn resolve_bucket_image(
    state: &AppState,
    headers: &HeaderMap,
    workload: &mut workloads::Model,
) {
    let Some((bucket_id, key)) = bucket_image_reference(&workload.image) else {
        return;
    };
    let key = utf8_percent_encode(key, KEY_ENCODE_SET);
    let path = presign_path(
        format!("/buckets/{bucket_id}/objects/presign-download/{key}"),
        headers,
    );
    let response = state
        .service_client
        .call_json(Service::ObjectStorage, Method::GET, &path, None)
        .await;
    match response {
        Ok((status, Some(body))) if status.is_success() => {
            if let Some(url) = body["url"].as_str() {
                workload.image = url.to_string();
            }
        }
        Ok((status, _)) => {
            tracing::warn!(workload = %workload.id, bucket = %bucket_id, status = %status, "failed to presign bucket image");
        }
        Err(error) => {
            tracing::warn!(workload = %workload.id, bucket = %bucket_id, error = %error, "failed to presign bucket image");
        }
    }
}
