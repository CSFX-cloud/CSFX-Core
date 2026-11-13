use base64::Engine;
use bcrypt::{hash, verify, DEFAULT_COST};
use rsa::rand_core::OsRng;
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey, EncodeRsaPublicKey},
    RsaPrivateKey, RsaPublicKey,
};
use std::sync::Arc;
use thiserror::Error;
use tokio::task::{spawn_blocking, JoinError};

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
    #[error("Blocking task failed: {0}")]
    JoinError(#[from] JoinError),
    #[error("Invalid encrypted data")]
    InvalidEncryptedData,
}

pub type CryptoResult<T> = Result<T, CryptoError>;

#[derive(Clone)]
pub struct RsaKeyPair {
    pub private_key: String,
    pub public_key: String,
    parsed: Arc<RsaPrivateKey>,
}

impl RsaKeyPair {
    pub async fn generate() -> CryptoResult<Self> {
        let private_key = spawn_blocking(|| RsaPrivateKey::new(&mut OsRng, 2048)).await??;
        let private_pem = private_key.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)?;
        Self::from_parsed(private_pem.to_string(), private_key)
    }

    pub fn from_private_key(private_pem: &str) -> CryptoResult<Self> {
        let private_key = RsaPrivateKey::from_pkcs1_pem(private_pem)?;
        Self::from_parsed(private_pem.to_string(), private_key)
    }

    fn from_parsed(private_key: String, parsed: RsaPrivateKey) -> CryptoResult<Self> {
        let public_key = RsaPublicKey::from(&parsed).to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)?;
        Ok(RsaKeyPair {
            private_key,
            public_key,
            parsed: Arc::new(parsed),
        })
    }
}

pub async fn decrypt_password(encrypted_password: &str, key: &RsaKeyPair) -> CryptoResult<String> {
    use rsa::sha2::Sha256;
    use rsa::Oaep;

    let encrypted_bytes = base64::engine::general_purpose::STANDARD.decode(encrypted_password)?;
    let private_key = Arc::clone(&key.parsed);
    let decrypted =
        spawn_blocking(move || private_key.decrypt(Oaep::new::<Sha256>(), &encrypted_bytes))
            .await??;

    String::from_utf8(decrypted).map_err(|_| CryptoError::InvalidEncryptedData)
}

pub async fn hash_password(password: &str, salt: &str) -> CryptoResult<String> {
    let salted_password = format!("{}{}", password, salt);
    Ok(spawn_blocking(move || hash(salted_password, DEFAULT_COST)).await??)
}

pub async fn verify_password(password: &str, salt: &str, hashed: &str) -> CryptoResult<bool> {
    let salted_password = format!("{}{}", password, salt);
    let hashed = hashed.to_string();
    Ok(spawn_blocking(move || verify(salted_password, &hashed)).await??)
}

pub fn generate_salt() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789";
    const SALT_LEN: usize = 32;

    (0..SALT_LEN)
        .map(|_| {
            let idx = rand::random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
