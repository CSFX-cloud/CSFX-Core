use axum::body::Bytes;
use flate2::{write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::io::Write;

const MIN_COMPRESSIBLE_BYTES: usize = 256;
const BROTLI_QUALITY: u32 = 5;
const BROTLI_WINDOW: u32 = 22;
const BROTLI_BUFFER: usize = 4096;
const ETAG_HASH_BYTES: usize = 16;
const SVELTEKIT_IMMUTABLE_DIR: &str = "_app/immutable/";
const VITE_ASSET_DIR: &str = "assets/";
const VITE_HASH_LENGTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    Identity,
    Gzip,
    Brotli,
}

impl Encoding {
    pub fn token(self) -> Option<&'static str> {
        match self {
            Encoding::Identity => None,
            Encoding::Gzip => Some("gzip"),
            Encoding::Brotli => Some("br"),
        }
    }
}

pub fn negotiate(accept_encoding: Option<&str>) -> Encoding {
    let Some(header) = accept_encoding else {
        return Encoding::Identity;
    };
    let accepts = |wanted: &str| {
        header.split(',').any(|item| {
            let mut parts = item.split(';');
            let name = parts.next().unwrap_or_default().trim();
            let refused = parts.any(|parameter| {
                parameter
                    .trim()
                    .strip_prefix("q=")
                    .and_then(|quality| quality.trim().parse::<f32>().ok())
                    .is_some_and(|quality| quality <= 0.0)
            });
            name.eq_ignore_ascii_case(wanted) && !refused
        })
    };
    if accepts("br") {
        Encoding::Brotli
    } else if accepts("gzip") {
        Encoding::Gzip
    } else {
        Encoding::Identity
    }
}

pub fn is_compressible(mime: &mime_guess::Mime) -> bool {
    let suffix = mime.suffix().map(|suffix| suffix.as_str());
    mime.type_() == mime_guess::mime::TEXT
        || matches!(
            mime.subtype().as_str(),
            "javascript" | "json" | "xml" | "wasm"
        )
        || matches!(suffix, Some("xml" | "json"))
}

pub fn compress(data: &[u8], encoding: Encoding) -> Option<Bytes> {
    if data.len() < MIN_COMPRESSIBLE_BYTES {
        return None;
    }
    let compressed = match encoding {
        Encoding::Identity => return None,
        Encoding::Gzip => gzip(data)?,
        Encoding::Brotli => brotli(data)?,
    };
    (compressed.len() < data.len()).then(|| Bytes::from(compressed))
}

fn gzip(data: &[u8]) -> Option<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).ok()?;
    encoder.finish().ok()
}

fn brotli(data: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(
            &mut output,
            BROTLI_BUFFER,
            BROTLI_QUALITY,
            BROTLI_WINDOW,
        );
        writer.write_all(data).ok()?;
    }
    Some(output)
}

pub fn content_hash(data: &[u8]) -> String {
    Sha256::digest(data)[..ETAG_HASH_BYTES]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn is_fingerprinted(path: &str) -> bool {
    path.starts_with(SVELTEKIT_IMMUTABLE_DIR) || is_vite_asset(path)
}

fn is_vite_asset(path: &str) -> bool {
    let Some(name) = path.strip_prefix(VITE_ASSET_DIR) else {
        return false;
    };
    let stem = name
        .rsplit('/')
        .next()
        .and_then(|file| file.split('.').next());
    stem.and_then(|stem| stem.rsplit('-').next())
        .is_some_and(looks_like_hash)
}

fn looks_like_hash(token: &str) -> bool {
    let has = |predicate: fn(&char) -> bool| token.chars().any(|character| predicate(&character));
    token.len() == VITE_HASH_LENGTH
        && token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && (has(char::is_ascii_digit)
            || (has(char::is_ascii_uppercase) && has(char::is_ascii_lowercase)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::io::Read;

    fn text(length: usize) -> Vec<u8> {
        "const value = 1;\n".repeat(length).into_bytes()
    }

    #[test]
    fn brotli_is_preferred_over_gzip() {
        assert_eq!(negotiate(Some("gzip, deflate, br")), Encoding::Brotli);
        assert_eq!(negotiate(Some("gzip, deflate")), Encoding::Gzip);
        assert_eq!(negotiate(Some("identity")), Encoding::Identity);
        assert_eq!(negotiate(None), Encoding::Identity);
    }

    #[test]
    fn refused_encodings_are_skipped() {
        assert_eq!(negotiate(Some("br;q=0, gzip")), Encoding::Gzip);
        assert_eq!(negotiate(Some("br;q=0.5")), Encoding::Brotli);
        assert_eq!(negotiate(Some("gzip;q=0")), Encoding::Identity);
    }

    #[test]
    fn only_text_like_types_are_compressible() {
        let mime = |path: &str| mime_guess::from_path(path).first_or_octet_stream();
        for path in [
            "a.js", "a.css", "a.html", "a.json", "a.svg", "a.wasm", "a.txt",
        ] {
            assert!(is_compressible(&mime(path)), "{path}");
        }
        for path in ["a.png", "a.woff2", "a.jpg", "a.mp4", "a.zip"] {
            assert!(!is_compressible(&mime(path)), "{path}");
        }
    }

    #[test]
    fn gzip_round_trips() {
        let data = text(200);
        let packed = compress(&data, Encoding::Gzip).unwrap();
        let mut unpacked = Vec::new();
        GzDecoder::new(&packed[..])
            .read_to_end(&mut unpacked)
            .unwrap();
        assert_eq!(unpacked, data);
    }

    #[test]
    fn brotli_round_trips_and_shrinks() {
        let data = text(200);
        let packed = compress(&data, Encoding::Brotli).unwrap();
        assert!(packed.len() < data.len());
        let mut unpacked = Vec::new();
        brotli::Decompressor::new(&packed[..], BROTLI_BUFFER)
            .read_to_end(&mut unpacked)
            .unwrap();
        assert_eq!(unpacked, data);
    }

    #[test]
    fn tiny_or_incompressible_data_stays_identity() {
        assert!(compress(b"tiny", Encoding::Brotli).is_none());
        assert!(compress(&text(200), Encoding::Identity).is_none());
    }

    #[test]
    fn content_hash_is_stable_and_content_dependent() {
        assert_eq!(content_hash(b"a"), content_hash(b"a"));
        assert_ne!(content_hash(b"a"), content_hash(b"b"));
        assert_eq!(content_hash(b"a").len(), ETAG_HASH_BYTES * 2);
    }

    #[test]
    fn fingerprinted_assets_are_recognized_by_toolchain_convention() {
        assert!(is_fingerprinted("_app/immutable/chunks/index.js"));
        assert!(is_fingerprinted("assets/index-BQ2f8aKz.js"));
        assert!(is_fingerprinted("assets/app-4f3a9c1e.css"));
        assert!(!is_fingerprinted("assets/logo.svg"));
        assert!(!is_fingerprinted("assets/bootstrap-datepicker.js"));
        assert!(!is_fingerprinted("assets/OpenSans-regular.woff2"));
        assert!(!is_fingerprinted("index.html"));
        assert!(!is_fingerprinted("static/app-4f3a9c1e.js"));
    }
}
