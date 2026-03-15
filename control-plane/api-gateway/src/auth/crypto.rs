use base64::Engine;
use bcrypt::{hash, verify, DEFAULT_COST};
use rand::rngs::OsRng;
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey, EncodeRsaPublicKey},
    RsaPrivateKey, RsaPublicKey,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("RSA error: {0}")]
    RsaError(#[from] rsa::Error),
    #[error("PKCS1 error: {0}")]
    Pkcs1Error(#[from] rsa::pkcs1::Error),
    #[error("Bcrypt error: {0}")]
    BcryptError(#[from] bcrypt::BcryptError),
    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("Invalid encrypted data")]
    InvalidEncryptedData,
}

pub type CryptoResult<T> = Result<T, CryptoError>;

pub struct RsaKeyPair {
    pub private_key: String,
    pub public_key: String,
}

impl RsaKeyPair {
    pub fn generate() -> CryptoResult<Self> {
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
        let public_key = RsaPublicKey::from(&private_key);

        let private_pem = private_key.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)?;
        let public_pem = public_key.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)?;

        Ok(RsaKeyPair {
            private_key: private_pem.to_string(),
            public_key: public_pem,
        })
    }

    pub fn from_private_key(private_pem: &str) -> CryptoResult<Self> {
        let private_key = RsaPrivateKey::from_pkcs1_pem(private_pem)?;
        let public_key = RsaPublicKey::from(&private_key);
        let public_pem = public_key.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)?;

        Ok(RsaKeyPair {
            private_key: private_pem.to_string(),
            public_key: public_pem,
        })
    }
}

pub fn decrypt_password(encrypted_password: &str, private_key: &str) -> CryptoResult<String> {
    use rsa::Oaep;
    use sha2::Sha256;

    let private_key = RsaPrivateKey::from_pkcs1_pem(private_key)?;
    let encrypted_bytes = base64::engine::general_purpose::STANDARD.decode(encrypted_password)?;

    // Use SHA-256 for RSA-OAEP padding
    let padding = Oaep::new::<Sha256>();
    let decrypted = private_key.decrypt(padding, &encrypted_bytes)?;

    String::from_utf8(decrypted).map_err(|_| CryptoError::InvalidEncryptedData)
}

pub fn hash_password(password: &str, salt: &str) -> CryptoResult<String> {
    let salted_password = format!("{}{}", password, salt);
    Ok(hash(salted_password, DEFAULT_COST)?)
}

pub fn verify_password(password: &str, salt: &str, hashed: &str) -> CryptoResult<bool> {
    let salted_password = format!("{}{}", password, salt);
    Ok(verify(salted_password, hashed)?)
}

pub fn encrypt_secret(plaintext: &str, key_b64: &str) -> CryptoResult<String> {
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
    use rand::RngCore;

    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|_| CryptoError::InvalidEncryptedData)?;
    if key_bytes.len() != 32 {
        return Err(CryptoError::InvalidEncryptedData);
    }

    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| CryptoError::InvalidEncryptedData)?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| CryptoError::InvalidEncryptedData)?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(base64::engine::general_purpose::STANDARD.encode(combined))
}

pub fn decrypt_secret(encoded: &str, key_b64: &str) -> CryptoResult<String> {
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};

    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|_| CryptoError::InvalidEncryptedData)?;
    if key_bytes.len() != 32 {
        return Err(CryptoError::InvalidEncryptedData);
    }

    let combined = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| CryptoError::InvalidEncryptedData)?;
    if combined.len() < 12 {
        return Err(CryptoError::InvalidEncryptedData);
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| CryptoError::InvalidEncryptedData)?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::InvalidEncryptedData)?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::InvalidEncryptedData)
}

pub fn generate_salt() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789";
    const SALT_LEN: usize = 32;
    let mut rng = rand::thread_rng();

    (0..SALT_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
