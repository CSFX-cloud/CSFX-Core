use base64::Engine;
use chrono::Utc;
use entity::{
    entities::system_settings, organization, role, user, user_organization, user_ssh_keys,
    Organization, Role, SystemSettings, User, UserOrganization, UserSshKeys,
};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait,
    DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, SqlErr,
    TransactionTrait,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::auth::crypto::{generate_salt, hash_password, verify_password, CryptoError};
use crate::console_pin::MIN_PIN_LENGTH;

pub const SETUP_KEY: &str = "setup.completed";
const LEGACY_ADMIN_NAME: &str = "admin@local.com";
const LEGACY_ADMIN_PASSWORD: &str = "admin";
const CA_CERT_PATH: &str = "/var/lib/csfx-cp/ca.crt";
const JOIN_REQUEST_PATH: &str = "/var/lib/csfx-cp/join-request";
const MIN_PASSWORD_LENGTH: usize = 12;
const MAX_ORGANIZATION_NAME_LENGTH: usize = 100;
const MAX_EMAIL_LENGTH: usize = 254;

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("setup already completed")]
    AlreadyCompleted,
    #[error("invalid setup code")]
    InvalidCode,
    #[error("setup code unavailable")]
    CodeUnavailable,
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    #[error("organization missing")]
    OrganizationMissing,
    #[error("admin role missing")]
    AdminRoleMissing,
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SetupInput {
    pub organization_name: String,
    pub admin_email: String,
    pub password: String,
    pub console_pin: String,
}

pub async fn is_completed(db: &impl ConnectionTrait) -> Result<bool, DbErr> {
    Ok(SystemSettings::find_by_id(SETUP_KEY)
        .one(db)
        .await?
        .is_some())
}

pub fn read_setup_code() -> Option<String> {
    let path = std::env::var("CSFX_SETUP_CODE_FILE").ok().or_else(|| {
        std::env::var("CREDENTIALS_DIRECTORY")
            .ok()
            .map(|dir| format!("{dir}/setup-code"))
    })?;
    let code = std::fs::read_to_string(path).ok()?;
    let code = code.trim();
    (!code.is_empty()).then(|| code.to_string())
}

pub fn is_valid_join_string(join_string: &str) -> bool {
    let parts: Vec<&str> = join_string.split('#').collect();
    let (endpoint, token, fingerprint) = match parts.as_slice() {
        [endpoint, token, fingerprint] => (endpoint, token, fingerprint),
        [endpoint, token, fingerprint, "worker" | "master"] => (endpoint, token, fingerprint),
        _ => return false,
    };
    let is_hex = |value: &str| {
        !value.is_empty() && value.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    };
    let is_endpoint = |value: &str| {
        !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-'))
    };
    endpoint.strip_prefix("csfx-join.").is_some_and(is_endpoint)
        && token.strip_prefix("csfx-bootstrap.").is_some_and(is_hex)
        && is_hex(fingerprint)
}

pub fn queue_join_request(join_string: &str) -> Result<(), SetupError> {
    if !is_valid_join_string(join_string) {
        return Err(SetupError::InvalidInput("join_string"));
    }
    let path = std::env::var("CSFX_JOIN_REQUEST_FILE").unwrap_or_else(|_| JOIN_REQUEST_PATH.into());
    let pending = format!("{path}.tmp");
    std::fs::write(&pending, join_string)?;
    std::fs::rename(&pending, &path)?;
    Ok(())
}

fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

pub fn code_matches(expected: &str, provided: &str) -> bool {
    let expected = Sha256::digest(normalize_code(expected));
    let provided = Sha256::digest(normalize_code(provided));
    expected
        .iter()
        .zip(provided.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

pub fn ca_pem() -> Option<String> {
    std::env::var("CSFX_CA_CERT_PEM")
        .ok()
        .or_else(|| std::fs::read_to_string(CA_CERT_PATH).ok())
}

pub fn ca_fingerprint() -> Option<String> {
    let pem = ca_pem()?;
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .ok()?;
    let hex: Vec<String> = Sha256::digest(der)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect();
    Some(hex.join(":"))
}

fn validate(input: &SetupInput) -> Result<(), SetupError> {
    let organization_name = input.organization_name.trim();
    if organization_name.is_empty()
        || organization_name.chars().count() > MAX_ORGANIZATION_NAME_LENGTH
    {
        return Err(SetupError::InvalidInput("organization_name"));
    }
    if !is_valid_email(&input.admin_email) {
        return Err(SetupError::InvalidInput("admin_email"));
    }
    if input.password.chars().count() < MIN_PASSWORD_LENGTH {
        return Err(SetupError::InvalidInput("password"));
    }
    if input.console_pin.chars().count() < MIN_PIN_LENGTH {
        return Err(SetupError::InvalidInput("console_pin"));
    }
    Ok(())
}

fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    email.len() <= MAX_EMAIL_LENGTH
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(|c| c.is_whitespace() || c.is_control())
}

async fn insert_marker(db: &impl ConnectionTrait, value: Value) -> Result<(), DbErr> {
    let marker = system_settings::ActiveModel {
        key: ActiveValue::Set(SETUP_KEY.to_string()),
        value: ActiveValue::Set(value),
        updated_at: ActiveValue::Set(Utc::now().fixed_offset()),
    };
    SystemSettings::insert(marker)
        .exec_without_returning(db)
        .await?;
    Ok(())
}

async fn rename_organization(db: &impl ConnectionTrait, name: &str) -> Result<Uuid, SetupError> {
    let org = Organization::find()
        .order_by_asc(organization::Column::CreatedAt)
        .one(db)
        .await?
        .ok_or(SetupError::OrganizationMissing)?;
    let org_id = org.id;
    let mut active: organization::ActiveModel = org.into();
    active.name = ActiveValue::Set(name.to_string());
    active.updated_at = ActiveValue::Set(Utc::now().naive_utc());
    active.update(db).await?;
    Ok(org_id)
}

async fn insert_admin(
    db: &impl ConnectionTrait,
    admin_id: Uuid,
    email: &str,
    password: &str,
) -> Result<(), SetupError> {
    let salt = generate_salt();
    let admin = user::ActiveModel {
        id: ActiveValue::Set(admin_id),
        name: ActiveValue::Set(email.to_string()),
        password: ActiveValue::Set(hash_password(password, &salt)?),
        salt: ActiveValue::Set(salt),
        email: ActiveValue::Set(Some(email.to_string())),
        gravatar_email: ActiveValue::NotSet,
        two_factor_secret: ActiveValue::NotSet,
        two_factor_enabled: ActiveValue::Set(false),
        force_password_change: ActiveValue::Set(false),
        auth_source: ActiveValue::Set("local".to_string()),
    };
    User::insert(admin).exec_without_returning(db).await?;
    Ok(())
}

async fn assign_admin_role(
    db: &impl ConnectionTrait,
    admin_id: Uuid,
    org_id: Uuid,
) -> Result<(), SetupError> {
    let admin_role = Role::find()
        .filter(role::Column::Name.eq("Admin"))
        .filter(role::Column::OrganizationId.eq(org_id))
        .one(db)
        .await?
        .ok_or(SetupError::AdminRoleMissing)?;
    let membership = user_organization::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        user_id: ActiveValue::Set(admin_id),
        organization_id: ActiveValue::Set(org_id),
        role_id: ActiveValue::Set(admin_role.id),
        joined_at: ActiveValue::Set(Utc::now().naive_utc()),
    };
    UserOrganization::insert(membership)
        .exec_without_returning(db)
        .await?;
    Ok(())
}

pub async fn complete_setup(
    db: &DatabaseConnection,
    input: SetupInput,
) -> Result<Uuid, SetupError> {
    validate(&input)?;
    let admin_id = Uuid::new_v4();
    let txn = db.begin().await?;
    insert_marker(
        &txn,
        json!({ "at": Utc::now().to_rfc3339(), "admin_id": admin_id }),
    )
    .await
    .map_err(|err| match err.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => SetupError::AlreadyCompleted,
        _ => SetupError::Database(err),
    })?;
    let org_id = rename_organization(&txn, input.organization_name.trim()).await?;
    insert_admin(&txn, admin_id, &input.admin_email, &input.password).await?;
    assign_admin_role(&txn, admin_id, org_id).await?;
    txn.commit().await?;
    Ok(admin_id)
}

async fn remove_unused_default_admin(db: &DatabaseConnection) -> Result<bool, SetupError> {
    let Some(admin) = User::find()
        .filter(user::Column::Name.eq(LEGACY_ADMIN_NAME))
        .one(db)
        .await?
    else {
        return Ok(false);
    };
    let untouched = admin.force_password_change
        && verify_password(LEGACY_ADMIN_PASSWORD, &admin.salt, &admin.password)?;
    if !untouched {
        return Ok(false);
    }
    let txn = db.begin().await?;
    UserSshKeys::delete_many()
        .filter(user_ssh_keys::Column::UserId.eq(admin.id))
        .exec(&txn)
        .await?;
    UserOrganization::delete_many()
        .filter(user_organization::Column::UserId.eq(admin.id))
        .exec(&txn)
        .await?;
    User::delete_by_id(admin.id).exec(&txn).await?;
    txn.commit().await?;
    Ok(true)
}

async fn mark_legacy(db: &DatabaseConnection) -> Result<(), DbErr> {
    let marker = system_settings::ActiveModel {
        key: ActiveValue::Set(SETUP_KEY.to_string()),
        value: ActiveValue::Set(json!({ "legacy": true, "at": Utc::now().to_rfc3339() })),
        updated_at: ActiveValue::Set(Utc::now().fixed_offset()),
    };
    SystemSettings::insert(marker)
        .on_conflict(
            OnConflict::column(system_settings::Column::Key)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await?;
    Ok(())
}

pub async fn reconcile_legacy(db: &DatabaseConnection) -> Result<(), SetupError> {
    if is_completed(db).await? {
        return Ok(());
    }
    if remove_unused_default_admin(db).await? {
        tracing::warn!(
            event = "legacy_default_admin_removed",
            user = LEGACY_ADMIN_NAME
        );
    }
    if UserOrganization::find().count(db).await? > 0 {
        mark_legacy(db).await?;
        tracing::info!(event = "setup_marked_legacy");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_comparison_ignores_case_and_separators() {
        assert!(code_matches("ABCD-EFGH", "abcd efgh"));
        assert!(!code_matches("ABCD-EFGH", "ABCD-EFGJ"));
    }

    #[test]
    fn join_string_validation_matches_console_format() {
        assert!(is_valid_join_string("csfx-join.10.0.0.5:8000#csfx-bootstrap.ab12#cd34"));
        assert!(is_valid_join_string("csfx-join.node-1:8000#csfx-bootstrap.ab12#cd34#master"));
        assert!(!is_valid_join_string("csfx-join.10.0.0.5:8000#csfx-bootstrap.ab12#cd34#other"));
        assert!(!is_valid_join_string("csfx-join.host#token#cd34"));
        assert!(!is_valid_join_string("csfx-join.host;rm#csfx-bootstrap.ab#cd"));
    }

    #[test]
    fn email_validation_rejects_malformed_addresses() {
        assert!(is_valid_email("admin@example.com"));
        assert!(!is_valid_email("admin@example"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("a b@example.com"));
    }
}
