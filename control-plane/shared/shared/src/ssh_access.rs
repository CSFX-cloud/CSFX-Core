use chrono::{DateTime, Utc};
use entity::{entities::system_settings, SystemSettings};
use sea_orm::{sea_query::OnConflict, ActiveValue, ConnectionTrait, DbErr, EntityTrait};
use serde::{Deserialize, Serialize};

pub const SSH_ACCESS_KEY: &str = "ssh.access";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshAccess {
    pub enabled: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

impl SshAccess {
    pub fn is_open_at(&self, now: DateTime<Utc>) -> bool {
        self.enabled && self.expires_at.is_none_or(|expires_at| expires_at > now)
    }

    pub fn is_open(&self) -> bool {
        self.is_open_at(Utc::now())
    }
}

pub async fn load(db: &impl ConnectionTrait) -> Result<SshAccess, DbErr> {
    let Some(setting) = SystemSettings::find_by_id(SSH_ACCESS_KEY).one(db).await? else {
        return Ok(SshAccess::default());
    };
    serde_json::from_value(setting.value)
        .map_err(|err| DbErr::Custom(format!("invalid ssh access setting: {err}")))
}

pub async fn save(db: &impl ConnectionTrait, access: &SshAccess) -> Result<(), DbErr> {
    let value = serde_json::to_value(access)
        .map_err(|err| DbErr::Custom(format!("unserializable ssh access setting: {err}")))?;
    let setting = system_settings::ActiveModel {
        key: ActiveValue::Set(SSH_ACCESS_KEY.to_string()),
        value: ActiveValue::Set(value),
        updated_at: ActiveValue::Set(Utc::now().fixed_offset()),
    };
    SystemSettings::insert(setting)
        .on_conflict(
            OnConflict::column(system_settings::Column::Key)
                .update_columns([
                    system_settings::Column::Value,
                    system_settings::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn closed_by_default() {
        assert!(!SshAccess::default().is_open());
    }

    #[test]
    fn expiry_closes_access() {
        let now = Utc::now();
        let access = SshAccess {
            enabled: true,
            expires_at: Some(now + Duration::hours(1)),
        };
        assert!(access.is_open_at(now));
        assert!(!access.is_open_at(now + Duration::hours(2)));
    }

    #[test]
    fn unlimited_stays_open() {
        let access = SshAccess {
            enabled: true,
            expires_at: None,
        };
        assert!(access.is_open_at(Utc::now() + Duration::days(3650)));
    }
}
