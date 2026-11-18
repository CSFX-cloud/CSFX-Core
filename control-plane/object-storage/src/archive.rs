use anyhow::{bail, Result};
use axum::body::Bytes;
use futures_util::{Stream, StreamExt};
use std::fmt::Display;
use tar::{EntryType, Header};

const BLOCK: usize = 512;
const NAME_FIELD: usize = 100;
const LONG_NAME_ENTRY: &[u8] = b"././@LongLink";

pub const TRAILER: [u8; BLOCK * 2] = [0; BLOCK * 2];

pub fn padding(size: u64) -> usize {
    (BLOCK - (size as usize % BLOCK)) % BLOCK
}

fn block(name: &[u8], size: u64, kind: EntryType) -> Header {
    let mut header = Header::new_gnu();
    header.set_entry_type(kind);
    header.set_size(size);
    header.set_mode(0o644);
    header.set_mtime(0);
    let field = &mut header.as_old_mut().name;
    field.fill(0);
    let length = name.len().min(NAME_FIELD);
    field[..length].copy_from_slice(&name[..length]);
    header.set_cksum();
    header
}

pub fn entry_header(key: &str, size: u64) -> Vec<u8> {
    let name = key.as_bytes();
    let mut out = Vec::with_capacity(BLOCK * 3);
    if name.len() > NAME_FIELD {
        let mut long_name = name.to_vec();
        long_name.push(0);
        let long_size = long_name.len() as u64;
        out.extend_from_slice(block(LONG_NAME_ENTRY, long_size, EntryType::GNULongName).as_bytes());
        out.extend_from_slice(&long_name);
        out.resize(out.len() + padding(long_size), 0);
    }
    out.extend_from_slice(block(name, size, EntryType::Regular).as_bytes());
    out
}

pub struct Entry {
    pub key: String,
    pub data: Vec<u8>,
}

pub struct EntryReader<S> {
    stream: S,
    buffer: Vec<u8>,
    max_entry_bytes: u64,
}

impl<S, E> EntryReader<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    pub fn new(stream: S, max_entry_bytes: u64) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            max_entry_bytes,
        }
    }

    async fn take(&mut self, length: usize) -> Result<Vec<u8>> {
        while self.buffer.len() < length {
            match self.stream.next().await {
                Some(chunk) => self
                    .buffer
                    .extend_from_slice(&chunk.map_err(|e| anyhow::anyhow!("{e}"))?),
                None => bail!("archive is truncated"),
            }
        }
        Ok(self.buffer.drain(..length).collect())
    }

    async fn take_padded(&mut self, size: u64) -> Result<Vec<u8>> {
        if size > self.max_entry_bytes {
            bail!("archive entry exceeds {} bytes", self.max_entry_bytes);
        }
        let mut data = self.take(size as usize + padding(size)).await?;
        data.truncate(size as usize);
        Ok(data)
    }

    pub async fn next_entry(&mut self) -> Result<Option<Entry>> {
        let mut long_name: Option<String> = None;
        loop {
            let raw = self.take(BLOCK).await?;
            if raw.iter().all(|byte| *byte == 0) {
                return Ok(None);
            }
            let header = Header::from_byte_slice(&raw);
            let size = header.size()?;
            let data = self.take_padded(size).await?;
            match header.entry_type() {
                EntryType::GNULongName => {
                    let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
                    long_name = Some(String::from_utf8(data[..end].to_vec())?);
                }
                EntryType::Regular | EntryType::Continuous => {
                    let key = match long_name.take() {
                        Some(name) => name,
                        None => String::from_utf8(header.path_bytes().into_owned())?,
                    };
                    return Ok(Some(Entry { key, data }));
                }
                _ => long_name = None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use std::io::Read;

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (key, data) in entries {
            out.extend(entry_header(key, data.len() as u64));
            out.extend_from_slice(data);
            out.resize(out.len() + padding(data.len() as u64), 0);
        }
        out.extend_from_slice(&TRAILER);
        out
    }

    fn sample() -> Vec<(String, Vec<u8>)> {
        let long_key = format!("deep/{}/object.bin", "d".repeat(180));
        vec![
            ("a.txt".to_string(), b"hello".to_vec()),
            (long_key, vec![9u8; 1000]),
            ("empty".to_string(), Vec::new()),
            ("exact-block".to_string(), vec![1u8; 512]),
        ]
    }

    #[test]
    fn standard_tar_reader_accepts_the_archive() {
        let owned = sample();
        let borrowed: Vec<(&str, &[u8])> = owned
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();
        let bytes = archive(&borrowed);
        let mut reader = tar::Archive::new(bytes.as_slice());
        let mut seen = Vec::new();
        for entry in reader.entries().unwrap() {
            let mut entry = entry.unwrap();
            let key = entry.path().unwrap().to_string_lossy().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            seen.push((key, data));
        }
        assert_eq!(seen, owned);
    }

    #[tokio::test]
    async fn entry_reader_round_trips_in_small_chunks() {
        let owned = sample();
        let borrowed: Vec<(&str, &[u8])> = owned
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();
        let bytes = archive(&borrowed);
        let chunks: Vec<Result<Bytes, String>> = bytes
            .chunks(333)
            .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
            .collect();
        let mut reader = EntryReader::new(stream::iter(chunks), 1 << 20);
        let mut seen = Vec::new();
        while let Some(entry) = reader.next_entry().await.unwrap() {
            seen.push((entry.key, entry.data));
        }
        assert_eq!(seen, owned);
    }

    #[tokio::test]
    async fn rejects_truncated_and_oversized_archives() {
        let bytes = archive(&[("a", &[7u8; 2000])]);
        let cut: Vec<Result<Bytes, String>> = vec![Ok(Bytes::copy_from_slice(&bytes[..1000]))];
        let mut truncated = EntryReader::new(stream::iter(cut), 1 << 20);
        assert!(truncated.next_entry().await.is_err());

        let full: Vec<Result<Bytes, String>> = vec![Ok(Bytes::from(bytes))];
        let mut oversized = EntryReader::new(stream::iter(full), 1000);
        assert!(oversized.next_entry().await.is_err());
    }
}
