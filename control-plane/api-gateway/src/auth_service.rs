use chrono::{DateTime, Duration, Utc};
use entity::{invalid_jwt, key, user, user_ssh_keys, InvalidJwt, Key, User, UserSshKeys};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, Set, TransactionTrait,
};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use thiserror::Error;
use totp_rs::{Algorithm, Secret, TOTP};
use tracing::warn;
use uuid::Uuid;

use crate::auth::{
    crypto::{
        decrypt_password, generate_salt, hash_password, verify_password, CryptoError, RsaKeyPair,
    },
    jwt::{create_jwt, TOKEN_LIFETIME_HOURS},
};

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sea_orm::DbErr),
    #[error("Crypto error: {0}")]
    CryptoError(#[from] CryptoError),
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("User not found")]
    UserNotFound,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("User already exists")]
    UserAlreadyExists,
    #[error("2FA required")]
    TwoFactorRequired,
    #[error("Invalid 2FA code")]
    InvalidTwoFactorCode,
    #[error("Password change required")]
    #[allow(dead_code)]
    PasswordChangeRequired,
}

pub type AuthResult<T> = Result<T, AuthError>;

pub const SESSION_CUTOFF_PREFIX: &str = "user-cutoff:";

pub fn session_cutoff_token(user_id: Uuid, cutoff: i64) -> String {
    format!("{SESSION_CUTOFF_PREFIX}{user_id}:{cutoff}")
}

static KEY_CACHE: LazyLock<RwLock<HashMap<String, Arc<RsaKeyPair>>>> =
    LazyLock::new(Default::default);

fn cached_key(key_name: &str) -> Option<Arc<RsaKeyPair>> {
    KEY_CACHE.read().ok()?.get(key_name).cloned()
}

fn cache_key(key_name: &str, key_pair: Arc<RsaKeyPair>) -> Arc<RsaKeyPair> {
    match KEY_CACHE.write() {
        Ok(mut cache) => Arc::clone(cache.entry(key_name.to_string()).or_insert(key_pair)),
        Err(_) => key_pair,
    }
}

#[derive(Clone)]
pub struct AuthService {
    db: DatabaseConnection,
}

impl AuthService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn login_user(
        &self,
        username: String,
        encrypted_password: String,
        two_factor_code: Option<String>,
    ) -> AuthResult<(String, bool, bool)> {
        // Find user
        let user = User::find()
            .filter(user::Column::Name.eq(&username))
            .one(&self.db)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        if user.auth_source != "local" {
            return Err(AuthError::InvalidCredentials);
        }

        // Get RSA private key for password decryption
        let rsa_key = self.get_or_create_rsa_key("main").await?;

        // Decrypt the password
        let password = decrypt_password(&encrypted_password, &rsa_key).await?;

        if !verify_password(&password, &user.salt, &user.password).await? {
            warn!(username = %username, "failed login attempt");
            return Err(AuthError::InvalidCredentials);
        }

        // Check if 2FA is enabled
        if user.two_factor_enabled {
            if let Some(code) = two_factor_code {
                if !self.verify_2fa_code(&user, &code)? {
                    return Err(AuthError::InvalidTwoFactorCode);
                }
            } else {
                return Err(AuthError::TwoFactorRequired);
            }
        }

        // Create JWT token
        let token = create_jwt(user.id, user.name)?;
        Ok((token, user.two_factor_enabled, user.force_password_change))
    }

    pub async fn logout_user(&self, jti: Uuid, exp: DateTime<Utc>) -> AuthResult<()> {
        let jwt_id = Uuid::new_v4();
        let invalid_jwt = invalid_jwt::ActiveModel {
            id: ActiveValue::Set(jwt_id),
            token: ActiveValue::Set(jti.to_string()),
            exp: ActiveValue::Set(exp.naive_utc()),
        };

        // Insert without retrieving last_insert_id (which doesn't work with UUID PKs in SQLite)
        InvalidJwt::insert(invalid_jwt)
            .exec_without_returning(&self.db)
            .await?;
        Ok(())
    }

    pub async fn delete_users(&self, user_ids: &[Uuid]) -> AuthResult<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let txn = self.db.begin().await?;
        UserSshKeys::delete_many()
            .filter(user_ssh_keys::Column::UserId.is_in(user_ids.iter().copied()))
            .exec(&txn)
            .await?;
        User::delete_many()
            .filter(user::Column::Id.is_in(user_ids.iter().copied()))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    pub async fn get_user_by_id(&self, user_id: Uuid) -> AuthResult<user::Model> {
        User::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or(AuthError::UserNotFound)
    }

    pub async fn get_or_create_rsa_key(&self, key_name: &str) -> AuthResult<Arc<RsaKeyPair>> {
        if let Some(cached) = cached_key(key_name) {
            return Ok(cached);
        }
        let stored = match self.find_key(key_name).await? {
            Some(stored) => stored,
            None => self.insert_new_key(key_name).await?,
        };
        let key_pair = Arc::new(RsaKeyPair::from_private_key(&stored.private_key)?);
        Ok(cache_key(key_name, key_pair))
    }

    async fn find_key(&self, key_name: &str) -> AuthResult<Option<key::Model>> {
        Ok(Key::find()
            .filter(key::Column::Name.eq(key_name))
            .one(&self.db)
            .await?)
    }

    async fn insert_new_key(&self, key_name: &str) -> AuthResult<key::Model> {
        let key_pair = RsaKeyPair::generate().await?;
        let new_key = key::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            name: ActiveValue::Set(key_name.to_string()),
            private_key: ActiveValue::Set(key_pair.private_key),
        };
        Key::insert(new_key)
            .on_conflict(
                OnConflict::column(key::Column::Name)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await?;
        self.find_key(key_name)
            .await?
            .ok_or(AuthError::DatabaseError(DbErr::RecordNotFound(
                key_name.to_string(),
            )))
    }

    pub async fn decrypt_client_password(&self, encrypted_password: &str) -> AuthResult<String> {
        let rsa_key = self.get_or_create_rsa_key("main").await?;
        Ok(decrypt_password(encrypted_password, &rsa_key).await?)
    }

    pub async fn get_public_key(&self, key_name: &str) -> AuthResult<String> {
        let key_pair = self.get_or_create_rsa_key(key_name).await?;
        Ok(key_pair.public_key.clone())
    }

    // 2FA Methods
    pub async fn generate_2fa_secret(
        &self,
        user_id: Uuid,
        encrypted_password: String,
    ) -> AuthResult<(String, String)> {
        let user = self.get_user_by_id(user_id).await?;
        self.verify_current_password(&user, &encrypted_password)
            .await?;

        // Generate a new TOTP secret using raw bytes
        let secret_bytes: Vec<u8> = (0..20).map(|_| rand::random::<u8>()).collect();
        let secret = Secret::Raw(secret_bytes);

        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret.to_bytes().unwrap(),
            Some("CSFX-Core".to_string()),
            user.name.clone(),
        )
        .unwrap();

        let secret_string = secret.to_encoded().to_string();
        let qr_code_url = totp.get_qr_base64().unwrap();

        // Store the secret in the database but don't enable 2FA yet
        let mut user_active: user::ActiveModel = user.into();
        user_active.two_factor_secret = Set(Some(secret_string.clone()));
        user_active.update(&self.db).await?;

        Ok((secret_string, qr_code_url))
    }

    pub async fn enable_2fa(&self, user_id: Uuid, code: String) -> AuthResult<()> {
        let user = self.get_user_by_id(user_id).await?;

        if user.two_factor_secret.is_none() {
            return Err(AuthError::InvalidTwoFactorCode);
        }

        // Verify the code before enabling
        if !self.verify_2fa_code(&user, &code)? {
            return Err(AuthError::InvalidTwoFactorCode);
        }

        // Enable 2FA
        let mut user_active: user::ActiveModel = user.into();
        user_active.two_factor_enabled = Set(true);
        user_active.update(&self.db).await?;

        Ok(())
    }

    pub async fn disable_2fa(&self, user_id: Uuid, code: String) -> AuthResult<()> {
        let user = self.get_user_by_id(user_id).await?;

        if !user.two_factor_enabled {
            return Ok(());
        }

        // Verify the code before disabling
        if !self.verify_2fa_code(&user, &code)? {
            return Err(AuthError::InvalidTwoFactorCode);
        }

        // Disable 2FA and remove secret
        let mut user_active: user::ActiveModel = user.into();
        user_active.two_factor_enabled = Set(false);
        user_active.two_factor_secret = Set(None);
        user_active.update(&self.db).await?;

        Ok(())
    }

    pub fn verify_2fa_code(&self, user: &user::Model, code: &str) -> AuthResult<bool> {
        if let Some(secret) = &user.two_factor_secret {
            let totp = TOTP::new(
                Algorithm::SHA1,
                6,
                1,
                30,
                Secret::Encoded(secret.clone()).to_bytes().unwrap(),
                Some("CSFX-Core".to_string()),
                user.name.clone(),
            )
            .unwrap();

            Ok(totp.check_current(code).unwrap_or(false))
        } else {
            Ok(false)
        }
    }

    // Password and Email Management
    async fn verify_current_password(
        &self,
        user: &user::Model,
        encrypted_password: &str,
    ) -> AuthResult<()> {
        if encrypted_password.is_empty() {
            return Err(AuthError::InvalidCredentials);
        }
        let rsa_key = self.get_or_create_rsa_key("main").await?;
        let password = decrypt_password(encrypted_password, &rsa_key).await?;
        if !verify_password(&password, &user.salt, &user.password).await? {
            return Err(AuthError::InvalidCredentials);
        }
        Ok(())
    }

    async fn revoke_user_sessions(&self, user_id: Uuid) -> AuthResult<()> {
        let now = Utc::now();
        let marker = invalid_jwt::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            token: ActiveValue::Set(session_cutoff_token(user_id, now.timestamp())),
            exp: ActiveValue::Set((now + Duration::hours(TOKEN_LIFETIME_HOURS)).naive_utc()),
        };
        InvalidJwt::insert(marker)
            .exec_without_returning(&self.db)
            .await?;
        Ok(())
    }

    pub async fn change_password(
        &self,
        user_id: Uuid,
        old_encrypted_password: String,
        new_encrypted_password: String,
    ) -> AuthResult<String> {
        let user = self.get_user_by_id(user_id).await?;
        let rsa_key = self.get_or_create_rsa_key("main").await?;

        if !user.force_password_change {
            self.verify_current_password(&user, &old_encrypted_password)
                .await?;
        }

        // Hash new password with new salt
        let new_salt = generate_salt();
        let new_password = decrypt_password(&new_encrypted_password, &rsa_key).await?;
        let hashed_password = hash_password(&new_password, &new_salt).await?;

        let username = user.name.clone();
        let mut user_active: user::ActiveModel = user.into();
        user_active.password = Set(hashed_password);
        user_active.salt = Set(new_salt);
        user_active.force_password_change = Set(false);
        user_active.update(&self.db).await?;

        self.revoke_user_sessions(user_id).await?;
        Ok(create_jwt(user_id, username)?)
    }

    pub async fn change_email(
        &self,
        user_id: Uuid,
        new_email: String,
        encrypted_password: String,
    ) -> AuthResult<()> {
        let user = self.get_user_by_id(user_id).await?;
        self.verify_current_password(&user, &encrypted_password)
            .await?;

        // Check if email is already in use
        if let Some(_existing) = User::find()
            .filter(user::Column::Email.eq(&new_email))
            .one(&self.db)
            .await?
        {
            return Err(AuthError::UserAlreadyExists);
        }

        let mut user_active: user::ActiveModel = user.into();
        user_active.email = Set(Some(new_email));
        user_active.update(&self.db).await?;

        Ok(())
    }

    pub async fn change_gravatar_email(
        &self,
        user_id: Uuid,
        gravatar_email: Option<String>,
    ) -> AuthResult<()> {
        let user = self.get_user_by_id(user_id).await?;

        let mut user_active: user::ActiveModel = user.into();
        user_active.gravatar_email = Set(gravatar_email);
        user_active.update(&self.db).await?;

        Ok(())
    }
}
