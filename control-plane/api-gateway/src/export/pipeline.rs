use axum::body::Bytes;
use futures_util::{stream, StreamExt};
use sha2::{Digest, Sha256};

use super::{
    crypto::{Decryptor, ExportKey},
    manifest::hex,
    sink::{Sink, Writer},
    ByteStream, ExportError,
};
use uuid::Uuid;

pub struct Stored {
    pub bytes: u64,
    pub sha256: String,
}

struct Pump {
    writer: Writer,
    hasher: Sha256,
    total: u64,
}

impl Pump {
    async fn push(&mut self, bytes: Bytes) -> Result<(), ExportError> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.hasher.update(&bytes);
        self.total += bytes.len() as u64;
        self.writer.write(bytes).await?;
        Ok(())
    }
}

async fn pump(
    state: &mut Pump,
    key: Option<&ExportKey>,
    mut source: ByteStream,
) -> Result<(), ExportError> {
    let mut encryptor = None;
    if let Some(key) = key {
        let (sealer, header) = key.encryptor()?;
        state.push(Bytes::from(header)).await?;
        encryptor = Some(sealer);
    }
    while let Some(chunk) = source.next().await {
        let chunk = chunk?;
        let out = match encryptor.as_mut() {
            Some(sealer) => Bytes::from(sealer.update(&chunk)?),
            None => chunk,
        };
        state.push(out).await?;
    }
    if let Some(sealer) = encryptor {
        state.push(Bytes::from(sealer.finish()?)).await?;
    }
    Ok(())
}

pub async fn store(
    sink: &Sink,
    export_id: Uuid,
    name: &str,
    key: Option<&ExportKey>,
    source: ByteStream,
) -> Result<Stored, ExportError> {
    let writer = sink.writer(export_id, name).await?;
    let mut state = Pump {
        writer,
        hasher: Sha256::new(),
        total: 0,
    };
    match pump(&mut state, key, source).await {
        Ok(()) => {
            let Pump {
                writer,
                hasher,
                total,
            } = state;
            writer.finish().await?;
            Ok(Stored {
                bytes: total,
                sha256: hex(&hasher.finalize()),
            })
        }
        Err(error) => {
            state.writer.abort().await;
            Err(error)
        }
    }
}

struct Verify {
    source: ByteStream,
    hasher: Sha256,
    decryptor: Option<Decryptor>,
    expected: String,
    name: String,
    done: bool,
}

async fn advance(mut state: Verify) -> Result<Option<(Bytes, Verify)>, ExportError> {
    while !state.done {
        let Some(chunk) = state.source.next().await else {
            state.done = true;
            let actual = hex(&std::mem::take(&mut state.hasher).finalize());
            if actual != state.expected {
                return Err(ExportError::ChecksumMismatch(state.name));
            }
            if let Some(decryptor) = state.decryptor.take() {
                let tail = decryptor.finish()?;
                if !tail.is_empty() {
                    return Ok(Some((Bytes::from(tail), state)));
                }
            }
            continue;
        };
        let chunk = chunk?;
        state.hasher.update(&chunk);
        let out = match state.decryptor.as_mut() {
            Some(decryptor) => Bytes::from(decryptor.update(&chunk)?),
            None => chunk,
        };
        if !out.is_empty() {
            return Ok(Some((out, state)));
        }
    }
    Ok(None)
}

pub fn verified(
    source: ByteStream,
    key: Option<&ExportKey>,
    expected: String,
    name: String,
) -> ByteStream {
    let state = Verify {
        source,
        hasher: Sha256::new(),
        decryptor: key.map(ExportKey::decryptor),
        expected,
        name,
        done: false,
    };
    Box::pin(stream::try_unfold(state, advance))
}

pub async fn collect(mut source: ByteStream) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::new();
    while let Some(chunk) = source.next().await {
        out.extend_from_slice(&chunk?);
    }
    Ok(out)
}

pub fn once(bytes: Vec<u8>) -> ByteStream {
    Box::pin(stream::once(async move { Ok(Bytes::from(bytes)) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::manifest::sha256_hex;

    #[tokio::test]
    async fn store_then_verified_round_trips_encrypted_and_plain() {
        let root = std::env::temp_dir().join(format!("csfx-pipe-{}", Uuid::new_v4()));
        let sink = Sink::Local(root.clone());
        let payload: Vec<u8> = (0..3_000_000u32).map(|n| (n % 253) as u8).collect();
        let (key, _) = ExportKey::create("pipeline passphrase").unwrap();
        for key in [None, Some(&key)] {
            let id = Uuid::new_v4();
            let stored = store(&sink, id, "data/blob.bin", key, once(payload.clone()))
                .await
                .unwrap();
            let source = sink.open(id, "data/blob.bin").await.unwrap();
            let opened = verified(source, key, stored.sha256.clone(), "blob".to_string());
            assert_eq!(collect(opened).await.unwrap(), payload);

            let wrong = verified(
                sink.open(id, "data/blob.bin").await.unwrap(),
                key,
                sha256_hex(b"other"),
                "blob".to_string(),
            );
            assert!(matches!(
                collect(wrong).await,
                Err(ExportError::ChecksumMismatch(_))
            ));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
