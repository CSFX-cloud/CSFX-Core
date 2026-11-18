use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::{
    pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::num::NonZeroU32;
use thiserror::Error;

pub const CHUNK_SIZE: usize = 1 << 20;
const MAGIC: &[u8; 8] = b"CSFXENC1";
const PREFIX_LENGTH: usize = 7;
const HEADER_LENGTH: usize = MAGIC.len() + PREFIX_LENGTH;
const TAG_LENGTH: usize = 16;
const SALT_LENGTH: usize = 16;
const KEY_LENGTH: usize = 32;
const ITERATIONS: u32 = 600_000;
const MAX_ITERATIONS: u32 = 10_000_000;
const ALGORITHM: &str = "aes-256-gcm-chunked";
const KDF: &str = "pbkdf2-hmac-sha256";
const KEY_CHECK_CONTEXT: &[u8] = b"csfx-export-key-check";

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("encryption parameters are not supported")]
    Parameters,
    #[error("passphrase does not match the export")]
    WrongKey,
    #[error("encrypted data is truncated or corrupt")]
    Corrupt,
    #[error("secure random source is unavailable")]
    Random,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptionInfo {
    pub algorithm: String,
    pub kdf: String,
    pub iterations: u32,
    pub salt: String,
    pub chunk_size: usize,
    pub key_check: String,
}

#[derive(Clone)]
pub struct ExportKey {
    cipher: Aes256Gcm,
}

fn derive(passphrase: &str, salt: &[u8], iterations: u32) -> Result<[u8; KEY_LENGTH], CryptoError> {
    let iterations = NonZeroU32::new(iterations).ok_or(CryptoError::Parameters)?;
    let mut key = [0u8; KEY_LENGTH];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        salt,
        passphrase.as_bytes(),
        &mut key,
    );
    Ok(key)
}

fn key_check(key: &[u8; KEY_LENGTH]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(KEY_CHECK_CONTEXT);
    hasher.update(key);
    STANDARD.encode(hasher.finalize())
}

fn nonce(prefix: &[u8], counter: u32, last: bool) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    bytes[..PREFIX_LENGTH].copy_from_slice(prefix);
    bytes[PREFIX_LENGTH..PREFIX_LENGTH + 4].copy_from_slice(&counter.to_be_bytes());
    bytes[11] = u8::from(last);
    bytes
}

fn random<const N: usize>() -> Result<[u8; N], CryptoError> {
    let mut bytes = [0u8; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| CryptoError::Random)?;
    Ok(bytes)
}

impl ExportKey {
    pub fn create(passphrase: &str) -> Result<(Self, EncryptionInfo), CryptoError> {
        Self::create_with_iterations(passphrase, ITERATIONS)
    }

    fn create_with_iterations(
        passphrase: &str,
        iterations: u32,
    ) -> Result<(Self, EncryptionInfo), CryptoError> {
        let salt = random::<SALT_LENGTH>()?;
        let key = derive(passphrase, &salt, iterations)?;
        let info = EncryptionInfo {
            algorithm: ALGORITHM.to_string(),
            kdf: KDF.to_string(),
            iterations,
            salt: STANDARD.encode(salt),
            chunk_size: CHUNK_SIZE,
            key_check: key_check(&key),
        };
        Ok((Self::from_key(&key)?, info))
    }

    pub fn open(passphrase: &str, info: &EncryptionInfo) -> Result<Self, CryptoError> {
        let supported = info.algorithm == ALGORITHM
            && info.kdf == KDF
            && info.chunk_size == CHUNK_SIZE
            && info.iterations <= MAX_ITERATIONS;
        if !supported {
            return Err(CryptoError::Parameters);
        }
        let salt = STANDARD
            .decode(&info.salt)
            .map_err(|_| CryptoError::Parameters)?;
        let key = derive(passphrase, &salt, info.iterations)?;
        if key_check(&key) != info.key_check {
            return Err(CryptoError::WrongKey);
        }
        Self::from_key(&key)
    }

    fn from_key(key: &[u8; KEY_LENGTH]) -> Result<Self, CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Parameters)?;
        Ok(Self { cipher })
    }

    pub fn encryptor(&self) -> Result<(Encryptor, Vec<u8>), CryptoError> {
        let prefix = random::<PREFIX_LENGTH>()?;
        let mut header = Vec::with_capacity(HEADER_LENGTH);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&prefix);
        let encryptor = Encryptor {
            cipher: self.cipher.clone(),
            prefix,
            counter: 0,
            buffer: Vec::with_capacity(CHUNK_SIZE + 1),
        };
        Ok((encryptor, header))
    }

    pub fn decryptor(&self) -> Decryptor {
        Decryptor {
            cipher: self.cipher.clone(),
            prefix: None,
            counter: 0,
            buffer: Vec::with_capacity(CHUNK_SIZE + TAG_LENGTH + 1),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let (mut encryptor, mut out) = self.encryptor()?;
        out.extend(encryptor.update(plaintext)?);
        out.extend(encryptor.finish()?);
        Ok(out)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut decryptor = self.decryptor();
        let mut out = decryptor.update(data)?;
        out.extend(decryptor.finish()?);
        Ok(out)
    }
}

pub struct Encryptor {
    cipher: Aes256Gcm,
    prefix: [u8; PREFIX_LENGTH],
    counter: u32,
    buffer: Vec<u8>,
}

impl Encryptor {
    fn seal(&mut self, chunk: &[u8], last: bool) -> Result<Vec<u8>, CryptoError> {
        let nonce = nonce(&self.prefix, self.counter, last);
        self.counter = self.counter.checked_add(1).ok_or(CryptoError::Parameters)?;
        self.cipher
            .encrypt(Nonce::from_slice(&nonce), chunk)
            .map_err(|_| CryptoError::Corrupt)
    }

    pub fn update(&mut self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.buffer.extend_from_slice(data);
        let mut out = Vec::new();
        while self.buffer.len() > CHUNK_SIZE {
            let chunk: Vec<u8> = self.buffer.drain(..CHUNK_SIZE).collect();
            out.extend(self.seal(&chunk, false)?);
        }
        Ok(out)
    }

    pub fn finish(mut self) -> Result<Vec<u8>, CryptoError> {
        let chunk = std::mem::take(&mut self.buffer);
        self.seal(&chunk, true)
    }
}

pub struct Decryptor {
    cipher: Aes256Gcm,
    prefix: Option<[u8; PREFIX_LENGTH]>,
    counter: u32,
    buffer: Vec<u8>,
}

impl Decryptor {
    fn open(&mut self, chunk: &[u8], last: bool) -> Result<Vec<u8>, CryptoError> {
        let prefix = self.prefix.ok_or(CryptoError::Corrupt)?;
        let nonce = nonce(&prefix, self.counter, last);
        self.counter = self.counter.checked_add(1).ok_or(CryptoError::Corrupt)?;
        self.cipher
            .decrypt(Nonce::from_slice(&nonce), chunk)
            .map_err(|_| CryptoError::Corrupt)
    }

    fn read_header(&mut self) -> Result<(), CryptoError> {
        if self.prefix.is_some() || self.buffer.len() < HEADER_LENGTH {
            return Ok(());
        }
        let header: Vec<u8> = self.buffer.drain(..HEADER_LENGTH).collect();
        let (magic, prefix) = header.split_at(MAGIC.len());
        if magic != MAGIC {
            return Err(CryptoError::Corrupt);
        }
        self.prefix = Some(prefix.try_into().map_err(|_| CryptoError::Corrupt)?);
        Ok(())
    }

    pub fn update(&mut self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.buffer.extend_from_slice(data);
        self.read_header()?;
        let mut out = Vec::new();
        while self.prefix.is_some() && self.buffer.len() > CHUNK_SIZE + TAG_LENGTH {
            let chunk: Vec<u8> = self.buffer.drain(..CHUNK_SIZE + TAG_LENGTH).collect();
            out.extend(self.open(&chunk, false)?);
        }
        Ok(out)
    }

    pub fn finish(mut self) -> Result<Vec<u8>, CryptoError> {
        let chunk = std::mem::take(&mut self.buffer);
        self.open(&chunk, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> (ExportKey, EncryptionInfo) {
        ExportKey::create_with_iterations("correct horse", 1000).unwrap()
    }

    #[test]
    fn roundtrips_across_chunk_boundaries() {
        let (key, _) = test_key();
        for length in [
            0,
            1,
            CHUNK_SIZE - 1,
            CHUNK_SIZE,
            CHUNK_SIZE + 1,
            2 * CHUNK_SIZE + 5,
        ] {
            let plaintext = vec![7u8; length];
            let sealed = key.encrypt(&plaintext).unwrap();
            assert_eq!(key.decrypt(&sealed).unwrap(), plaintext);
        }
    }

    #[test]
    fn rejects_tampering_and_truncation() {
        let (key, _) = test_key();
        let plaintext = vec![1u8; CHUNK_SIZE + 10];
        let sealed = key.encrypt(&plaintext).unwrap();

        let mut flipped = sealed.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 1;
        assert!(key.decrypt(&flipped).is_err());

        let first_chunk_end = HEADER_LENGTH + CHUNK_SIZE + TAG_LENGTH;
        assert!(key.decrypt(&sealed[..first_chunk_end]).is_err());
        assert!(key.decrypt(&sealed[..MAGIC.len()]).is_err());
    }

    #[test]
    fn streams_in_arbitrary_slices() {
        let (key, _) = test_key();
        let plaintext: Vec<u8> = (0..(2 * CHUNK_SIZE + 77))
            .map(|n| (n % 251) as u8)
            .collect();
        let (mut encryptor, mut sealed) = key.encryptor().unwrap();
        for piece in plaintext.chunks(300_001) {
            sealed.extend(encryptor.update(piece).unwrap());
        }
        sealed.extend(encryptor.finish().unwrap());
        let mut decryptor = key.decryptor();
        let mut opened = Vec::new();
        for piece in sealed.chunks(65_537) {
            opened.extend(decryptor.update(piece).unwrap());
        }
        opened.extend(decryptor.finish().unwrap());
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn opens_only_with_matching_passphrase() {
        let (key, info) = test_key();
        let sealed = key.encrypt(b"payload").unwrap();
        let reopened = ExportKey::open("correct horse", &info).unwrap();
        assert_eq!(reopened.decrypt(&sealed).unwrap(), b"payload");
        assert!(matches!(
            ExportKey::open("wrong", &info),
            Err(CryptoError::WrongKey)
        ));
    }
}
