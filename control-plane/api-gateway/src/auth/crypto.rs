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
