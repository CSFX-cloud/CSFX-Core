use entity::user;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, DbErr};
use shared::ssh_access::{self, SshAccess};
use std::io::BufRead;
use thiserror::Error;

use crate::admins::admin_users;
use crate::auth::crypto::{generate_salt, hash_password, verify_password, CryptoError};
use crate::console_pin::{self, PinError};

const TEMP_PASSWORD_LENGTH: usize = 20;
const TEMP_PASSWORD_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";

#[derive(Debug, Error)]
enum RecoveryError {
    #[error("{0}")]
    Pin(#[from] PinError),
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("user not found")]
    UserNotFound,
    #[error("admin credentials rejected")]
    NotAdmin,
    #[error("console pin already provisioned")]
    AlreadyProvisioned,
    #[error("missing input line")]
    MissingInput,
}

impl RecoveryError {
    fn exit_code(&self) -> i32 {
        match self {
            RecoveryError::Pin(PinError::Rejected) => 1,
            RecoveryError::Pin(PinError::Locked(_)) => 2,
            RecoveryError::Pin(PinError::NotProvisioned) => 3,
            _ => 4,
        }
    }
}

pub async fn dispatch(args: &[String]) -> Option<i32> {
    let command = args.first()?.as_str();
    let outcome = match command {
        "--console-auth" => console_auth(),
        "--console-pin-change" => console_pin_change(),
        "--console-pin-init" => console_pin_init().await,
        "--ssh-disable" => ssh_disable().await,
        "--reset-admin" => reset_admin(args.get(1).map(String::as_str)).await,
        _ => return None,
    };
    Some(report(outcome))
}

fn report(outcome: Result<(), RecoveryError>) -> i32 {
    match outcome {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("[ERROR] recovery command failed reason=\"{err}\"");
            err.exit_code()
        }
    }
}

fn read_line() -> Result<String, RecoveryError> {
    let mut line = String::new();
    let read = std::io::stdin().lock().read_line(&mut line)?;
    if read == 0 {
        return Err(RecoveryError::MissingInput);
    }
    Ok(line.trim_end_matches(['\n', '\r']).to_string())
}

fn console_auth() -> Result<(), RecoveryError> {
    Ok(console_pin::verify(&read_line()?)?)
}

fn console_pin_change() -> Result<(), RecoveryError> {
    let current = read_line()?;
    let new_pin = read_line()?;
    console_pin::verify(&current)?;
    println!("recovery_key={}", console_pin::provision(&new_pin)?);
    Ok(())
}

async fn connect() -> Result<DatabaseConnection, RecoveryError> {
    Ok(crate::db::establish_connection().await?)
}

async fn ssh_disable() -> Result<(), RecoveryError> {
    let db = connect().await?;
    ssh_access::save(&db, &SshAccess::default()).await?;
    println!("ssh_access=disabled");
    Ok(())
}

async fn console_pin_init() -> Result<(), RecoveryError> {
    let email = read_line()?;
    let password = read_line()?;
    let new_pin = read_line()?;
    if console_pin::is_provisioned() {
        return Err(RecoveryError::AlreadyProvisioned);
    }
    let db = connect().await?;
    let admin = admin_users(&db)
        .await?
        .into_iter()
        .find(|candidate| candidate.name == email || candidate.email.as_deref() == Some(&email))
        .ok_or(RecoveryError::NotAdmin)?;
    if !verify_password(&password, &admin.salt, &admin.password)? {
        return Err(RecoveryError::NotAdmin);
    }
    println!("recovery_key={}", console_pin::provision(&new_pin)?);
    Ok(())
}

fn generate_temp_password() -> String {
    (0..TEMP_PASSWORD_LENGTH)
        .map(|_| {
            TEMP_PASSWORD_ALPHABET[rand::random_range(0..TEMP_PASSWORD_ALPHABET.len())] as char
        })
        .collect()
}

async fn reset_admin(email: Option<&str>) -> Result<(), RecoveryError> {
    let db = connect().await?;
    let Some(email) = email else {
        admin_users(&db)
            .await?
            .iter()
            .for_each(|admin| println!("admin={}", admin.name));
        return Ok(());
    };
    let target = admin_users(&db)
        .await?
        .into_iter()
        .find(|candidate| candidate.name == email)
        .ok_or(RecoveryError::UserNotFound)?;
    let temp_password = generate_temp_password();
    let salt = generate_salt();
    let mut active: user::ActiveModel = target.into();
    active.password = ActiveValue::Set(hash_password(&temp_password, &salt)?);
    active.salt = ActiveValue::Set(salt);
    active.force_password_change = ActiveValue::Set(true);
    active.two_factor_enabled = ActiveValue::Set(false);
    active.two_factor_secret = ActiveValue::Set(None);
    active.update(&db).await?;
    println!("temporary_password={temp_password}");
    Ok(())
}
