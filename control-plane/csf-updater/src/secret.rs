use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use anyhow::{bail, Result};
use base64::Engine;

pub fn decrypt_secret(encoded: &str, key_b64: &str) -> Result<String> {
    let key_bytes = base64::engine::general_purpose::STANDARD.decode(key_b64)?;
    if key_bytes.len() != 32 {
        bail!("invalid encryption key length");
    }

    let combined = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    if combined.len() < 12 {
        bail!("invalid ciphertext");
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| anyhow::anyhow!("cipher init failed: {}", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decryption failed: {}", e))?;

    Ok(String::from_utf8(plaintext)?)
}
