use entity::{entities::system_settings, SystemSettings};
use sea_orm::{
    sea_query::OnConflict, ActiveValue::Set, ColumnTrait, DbConn, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{s3::S3Config, sink::Sink, ExportError};
use crate::oidc::secrets;

const TARGET_KEY_PREFIX: &str = "export.target.";
const DEFAULT_KEY: &str = "export.default_target";
const MAX_NAME_LENGTH: usize = 63;
const MIN_PASSPHRASE_LENGTH: usize = 12;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Destination {
    Local {
        path: String,
    },
    S3 {
        endpoint: String,
        region: String,
        bucket: String,
        prefix: String,
        access_key_id: String,
        encrypted_secret_access_key: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredTarget {
    #[serde(flatten)]
    destination: Destination,
    encrypted_passphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DestinationInput {
    Local {
        path: String,
    },
    S3 {
        endpoint: String,
        region: String,
        bucket: String,
        #[serde(default)]
        prefix: String,
        access_key_id: String,
        secret_access_key: String,
    },
}

#[derive(Debug, Serialize)]
pub struct TargetView {
    pub name: String,
    pub kind: &'static str,
    pub location: String,
    pub encrypted: bool,
    pub default: bool,
}

pub struct Target {
    pub sink: Sink,
    pub passphrase: Option<String>,
}

fn invalid(message: &str) -> ExportError {
    ExportError::InvalidTarget(message.to_string())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LENGTH
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn valid_prefix(prefix: &str) -> bool {
    prefix
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
        && !prefix.split('/').any(|part| part == "..")
}

fn seal_destination(input: DestinationInput) -> Result<Destination, ExportError> {
    match input {
        DestinationInput::Local { path } => {
            if !PathBuf::from(&path).is_absolute() {
                return Err(invalid("path must be absolute"));
            }
            Ok(Destination::Local { path })
        }
        DestinationInput::S3 {
            endpoint,
            region,
            bucket,
            prefix,
            access_key_id,
            secret_access_key,
        } => {
            let complete = [&region, &bucket, &access_key_id, &secret_access_key]
                .iter()
                .all(|value| !value.is_empty());
            if !complete {
                return Err(invalid("region, bucket and credentials are required"));
            }
            if !(endpoint.starts_with("https://") || endpoint.starts_with("http://")) {
                return Err(invalid("endpoint must start with http:// or https://"));
            }
            if !valid_prefix(&prefix) {
                return Err(invalid("prefix contains unsupported characters"));
            }
            Ok(Destination::S3 {
                endpoint,
                region,
                bucket,
                prefix,
                access_key_id,
                encrypted_secret_access_key: secrets::encrypt(&secret_access_key)?,
            })
        }
    }
}

fn open_sink(destination: &Destination) -> Result<Sink, ExportError> {
    match destination {
        Destination::Local { path } => Ok(Sink::Local(PathBuf::from(path))),
        Destination::S3 {
            endpoint,
            region,
            bucket,
            prefix,
            access_key_id,
            encrypted_secret_access_key,
        } => Ok(Sink::s3(S3Config {
            endpoint: endpoint.clone(),
            region: region.clone(),
            bucket: bucket.clone(),
            prefix: prefix.clone(),
            access_key_id: access_key_id.clone(),
            secret_access_key: secrets::decrypt(encrypted_secret_access_key)?,
        })),
    }
}

fn view(name: &str, stored: &StoredTarget, default: bool) -> TargetView {
    let (kind, location) = match &stored.destination {
        Destination::Local { path } => ("local", path.clone()),
        Destination::S3 {
            endpoint,
            bucket,
            prefix,
            ..
        } => ("s3", format!("{endpoint}/{bucket}/{prefix}")),
    };
    TargetView {
        name: name.to_string(),
        kind,
        location,
        encrypted: stored.encrypted_passphrase.is_some(),
        default,
    }
}

fn target_key(name: &str) -> String {
    format!("{TARGET_KEY_PREFIX}{name}")
}

async fn upsert_setting(
    db: &DbConn,
    key: String,
    value: serde_json::Value,
) -> Result<(), ExportError> {
    let row = system_settings::ActiveModel {
        key: Set(key),
        value: Set(value),
        updated_at: Set(chrono::Utc::now().into()),
    };
    SystemSettings::insert(row)
        .on_conflict(
            OnConflict::column(system_settings::Column::Key)
                .update_columns([
                    system_settings::Column::Value,
                    system_settings::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

async fn default_name(db: &DbConn) -> Result<Option<String>, ExportError> {
    let Some(setting) = SystemSettings::find_by_id(DEFAULT_KEY).one(db).await? else {
        return Ok(None);
    };
    Ok(serde_json::from_value(setting.value)?)
}

async fn find_stored(db: &DbConn, name: &str) -> Result<StoredTarget, ExportError> {
    let setting = SystemSettings::find_by_id(target_key(name))
        .one(db)
        .await?
        .ok_or_else(|| ExportError::TargetNotFound(name.to_string()))?;
    Ok(serde_json::from_value(setting.value)?)
}

pub async fn list(db: &DbConn) -> Result<Vec<TargetView>, ExportError> {
    let default = default_name(db).await?;
    let rows = SystemSettings::find()
        .filter(system_settings::Column::Key.starts_with(TARGET_KEY_PREFIX))
        .order_by_asc(system_settings::Column::Key)
        .all(db)
        .await?;
    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let name = row.key.trim_start_matches(TARGET_KEY_PREFIX).to_string();
        let stored: StoredTarget = serde_json::from_value(row.value)?;
        views.push(view(&name, &stored, default.as_deref() == Some(&name)));
    }
    Ok(views)
}

pub async fn put(
    db: &DbConn,
    name: &str,
    input: DestinationInput,
    passphrase: Option<String>,
    make_default: bool,
) -> Result<TargetView, ExportError> {
    if !valid_name(name) {
        return Err(invalid("name must be 1-63 characters of a-z, 0-9, - or _"));
    }
    if passphrase
        .as_deref()
        .is_some_and(|p| p.len() < MIN_PASSPHRASE_LENGTH)
    {
        return Err(invalid("passphrase must be at least 12 characters"));
    }
    let stored = StoredTarget {
        destination: seal_destination(input)?,
        encrypted_passphrase: passphrase.as_deref().map(secrets::encrypt).transpose()?,
    };
    upsert_setting(db, target_key(name), serde_json::to_value(&stored)?).await?;
    let current = default_name(db).await?;
    let is_default = make_default || current.is_none();
    if is_default {
        upsert_setting(db, DEFAULT_KEY.to_string(), serde_json::json!(name)).await?;
    }
    Ok(view(
        name,
        &stored,
        is_default || current.as_deref() == Some(name),
    ))
}

pub async fn remove(db: &DbConn, name: &str) -> Result<(), ExportError> {
    let deleted = SystemSettings::delete_by_id(target_key(name))
        .exec(db)
        .await?;
    if deleted.rows_affected == 0 {
        return Err(ExportError::TargetNotFound(name.to_string()));
    }
    if default_name(db).await?.as_deref() == Some(name) {
        SystemSettings::delete_by_id(DEFAULT_KEY).exec(db).await?;
    }
    Ok(())
}

pub async fn load(db: &DbConn, name: Option<&str>) -> Result<Target, ExportError> {
    let name = match name {
        Some(name) => name.to_string(),
        None => default_name(db).await?.ok_or(ExportError::NotConfigured)?,
    };
    let stored = find_stored(db, &name).await?;
    Ok(Target {
        sink: open_sink(&stored.destination)?,
        passphrase: stored
            .encrypted_passphrase
            .as_deref()
            .map(secrets::decrypt)
            .transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names_and_prefixes() {
        assert!(valid_name("offsite-1"));
        assert!(!valid_name(""));
        assert!(!valid_name("Upper"));
        assert!(!valid_name("a/b"));
        assert!(valid_prefix("csfx/prod"));
        assert!(!valid_prefix("../x"));
        assert!(!valid_prefix("a b"));
    }

    #[test]
    fn rejects_relative_local_path_and_bad_endpoint() {
        assert!(seal_destination(DestinationInput::Local {
            path: "relative".to_string()
        })
        .is_err());
        let s3 = |endpoint: &str| DestinationInput::S3 {
            endpoint: endpoint.to_string(),
            region: "r".to_string(),
            bucket: "b".to_string(),
            prefix: String::new(),
            access_key_id: "k".to_string(),
            secret_access_key: "s".to_string(),
        };
        assert!(seal_destination(s3("ftp://x")).is_err());
    }
}
