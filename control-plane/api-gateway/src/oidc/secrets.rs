use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::auth::jwt::get_jwt_secret;

const NONCE_LENGTH: usize = 12;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("encryption failed")]
    Encrypt,
    #[error("stored secret is malformed or the key changed")]
    Decrypt,
}

fn cipher() -> Aes256Gcm {
    let material = std::env::var("CSFX_SECRET_KEY").unwrap_or_else(|_| get_jwt_secret());
    let digest = Sha256::digest(format!("csfx-secret-store:{material}").as_bytes());
    Aes256Gcm::new_from_slice(&digest).expect("sha256 digest is a valid aes-256 key")
}

pub fn encrypt(plaintext: &str) -> Result<String, SecretError> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher()
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| SecretError::Encrypt)?;
    let mut payload = nonce.to_vec();
    payload.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(payload))
}

pub fn decrypt(encoded: &str) -> Result<String, SecretError> {
    let payload = STANDARD.decode(encoded).map_err(|_| SecretError::Decrypt)?;
    if payload.len() <= NONCE_LENGTH {
        return Err(SecretError::Decrypt);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_LENGTH);
    let plaintext = cipher()
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| SecretError::Decrypt)?;
    String::from_utf8(plaintext).map_err(|_| SecretError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper_detection() {
        let encoded = encrypt("client-secret").unwrap();
        assert_eq!(decrypt(&encoded).unwrap(), "client-secret");
        let mut payload = STANDARD.decode(&encoded).unwrap();
        let last = payload.len() - 1;
        payload[last] ^= 1;
        assert!(decrypt(&STANDARD.encode(payload)).is_err());
    }
}
