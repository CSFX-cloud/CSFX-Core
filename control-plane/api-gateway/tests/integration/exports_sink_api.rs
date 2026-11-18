use api_gateway::export::{pipeline, s3::S3Config, sink::Sink};
use uuid::Uuid;

use crate::fake_s3::FakeS3;

const PART_SIZE: usize = 64 << 20;

fn s3_sink(endpoint: String) -> Sink {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    Sink::s3(S3Config {
        endpoint,
        region: "us-east-1".to_string(),
        bucket: "backups".to_string(),
        prefix: "csfx/sinks".to_string(),
        access_key_id: "key".to_string(),
        secret_access_key: "secret".to_string(),
    })
}

#[tokio::test]
async fn large_artifacts_are_uploaded_in_parts_and_verified_on_read() {
    let (endpoint, fake) = FakeS3::start().await;
    let sink = s3_sink(endpoint);
    let payload: Vec<u8> = (0..(PART_SIZE + 6 * 1024 * 1024))
        .map(|n| (n % 251) as u8)
        .collect();
    let id = Uuid::new_v4();
    let stored = pipeline::store(
        &sink,
        id,
        "big/blob.bin",
        None,
        pipeline::once(payload.clone()),
    )
    .await
    .unwrap();
    assert_eq!(stored.bytes as usize, payload.len());
    assert_eq!(fake.uploaded_parts(), 2);
    let key = format!("/backups/csfx/sinks/{id}/big/blob.bin");
    assert_eq!(fake.object(&key).unwrap(), payload);

    let source = sink.open(id, "big/blob.bin").await.unwrap();
    let opened = pipeline::verified(source, None, stored.sha256, "blob".to_string());
    assert_eq!(pipeline::collect(opened).await.unwrap(), payload);
}

#[tokio::test]
async fn small_artifacts_use_a_single_put_and_probe_cleans_up() {
    let (endpoint, fake) = FakeS3::start().await;
    let sink = s3_sink(endpoint);
    let id = Uuid::new_v4();
    pipeline::store(
        &sink,
        id,
        "small.json",
        None,
        pipeline::once(b"[]".to_vec()),
    )
    .await
    .unwrap();
    assert_eq!(fake.uploaded_parts(), 0);
    sink.probe().await.unwrap();
    assert_eq!(fake.keys().len(), 1);
}
