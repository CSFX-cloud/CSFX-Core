use chrono::{DateTime, Duration, Utc};
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::openbao::{OpenBao, OpenBaoError};

const KV_MOUNT: &str = "csfx-kv";
const MAX_SECRET_NAME_LENGTH: usize = 127;
const MAX_PRINCIPAL_LENGTH: usize = 63;
const SECRET_ID_WRAP_TTL: &str = "60s";

#[derive(Clone, Copy)]
pub struct VaultScope {
    pub resource_group_id: Uuid,
    pub vault_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct SecretSummary {
    pub name: String,
    pub version: u64,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub used_by: Vec<String>,
    pub deleted: bool,
}

pub struct SecretValue {
    pub value: String,
    pub version: u64,
}

pub struct SecretWrite {
    pub value: String,
    pub expires_at: Option<String>,
    pub used_by: Vec<String>,
}

pub fn validate_secret_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || name.len() > MAX_SECRET_NAME_LENGTH {
        return Err("name must be between 1 and 127 characters");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("name may only contain letters, digits, underscore and hyphen");
    }
    Ok(())
}

impl VaultScope {
    fn prefix(&self) -> String {
        format!("rg/{}/{}", self.resource_group_id, self.vault_id)
    }

    fn data(&self, name: &str) -> String {
        format!("{KV_MOUNT}/data/{}/{name}", self.prefix())
    }

    fn metadata(&self, name: &str) -> String {
        format!("{KV_MOUNT}/metadata/{}/{name}", self.prefix())
    }

    fn listing(&self) -> String {
        format!("{KV_MOUNT}/metadata/{}/?list=true", self.prefix())
    }

    fn actions(&self, action: &str, name: &str) -> String {
        format!("{KV_MOUNT}/{action}/{}/{name}", self.prefix())
    }

    pub fn principal_role(&self, principal: &str) -> String {
        format!("csfx-vault-{}-{principal}", self.vault_id)
    }

    pub fn principal_policy(&self, scopes: &[String]) -> String {
        let prefix = self.prefix();
        scopes
            .iter()
            .map(|scope| {
                format!(
                    "path \"{KV_MOUNT}/data/{prefix}/{scope}\" {{\n  capabilities = [\"read\"]\n}}\n"
                )
            })
            .collect()
    }
}

pub fn validate_principal(principal: &str) -> Result<(), &'static str> {
    let valid = !principal.is_empty()
        && principal.len() <= MAX_PRINCIPAL_LENGTH
        && principal
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
    if valid {
        Ok(())
    } else {
        Err("principal may only contain letters, digits, underscore, hyphen and dot")
    }
}

pub fn validate_scope(scope: &str) -> Result<(), &'static str> {
    let valid = !scope.is_empty()
        && scope.len() <= MAX_SECRET_NAME_LENGTH
        && scope
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '*');
    if valid {
        Ok(())
    } else {
        Err("scope may only contain letters, digits, underscore, hyphen and asterisk")
    }
}

pub fn scope_matches(scope: &str, name: &str) -> bool {
    match scope.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => scope == name,
    }
}

fn parse_version(response: &Value) -> Result<u64, OpenBaoError> {
    response
        .pointer("/data/version")
        .and_then(Value::as_u64)
        .ok_or(OpenBaoError::Decode)
}

pub fn summarize(name: &str, metadata: &Value) -> Result<SecretSummary, OpenBaoError> {
    let data = metadata.get("data").ok_or(OpenBaoError::Decode)?;
    let version = data
        .get("current_version")
        .and_then(Value::as_u64)
        .ok_or(OpenBaoError::Decode)?;
    let current = data.pointer(&format!("/versions/{version}"));
    let custom = data.get("custom_metadata");
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_string);
    Ok(SecretSummary {
        name: name.to_string(),
        version,
        updated_at: text(data.get("updated_time")).unwrap_or_default(),
        expires_at: text(custom.and_then(|c| c.get("expires_at"))).filter(|v| !v.is_empty()),
        used_by: text(custom.and_then(|c| c.get("used_by")))
            .map(|joined| {
                joined
                    .split(',')
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        deleted: current
            .and_then(|v| v.get("deletion_time"))
            .and_then(Value::as_str)
            .is_some_and(|time| !time.is_empty()),
    })
}

#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub version: u64,
    pub created_at: String,
    pub deleted: bool,
    pub destroyed: bool,
}

pub fn version_list(metadata: &Value) -> Result<Vec<VersionInfo>, OpenBaoError> {
    let versions = metadata
        .pointer("/data/versions")
        .and_then(Value::as_object)
        .ok_or(OpenBaoError::Decode)?;
    let mut listed: Vec<VersionInfo> = versions
        .iter()
        .filter_map(|(number, entry)| {
            Some(VersionInfo {
                version: number.parse().ok()?,
                created_at: entry.get("created_time")?.as_str()?.to_string(),
                deleted: entry
                    .get("deletion_time")
                    .and_then(Value::as_str)
                    .is_some_and(|time| !time.is_empty()),
                destroyed: entry
                    .get("destroyed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect();
    listed.sort_by_key(|info| std::cmp::Reverse(info.version));
    Ok(listed)
}

pub fn deletion_overdue(metadata: &Value, retention_days: i32, now: DateTime<Utc>) -> Option<bool> {
    let data = metadata.get("data")?;
    let version = data.get("current_version")?.as_u64()?;
    let deleted_at = data
        .pointer(&format!("/versions/{version}/deletion_time"))?
        .as_str()
        .filter(|time| !time.is_empty())?;
    let deleted_at = DateTime::parse_from_rfc3339(deleted_at)
        .ok()?
        .with_timezone(&Utc);
    Some(now - deleted_at >= Duration::days(i64::from(retention_days)))
}

pub async fn write_secret(
    bao: &OpenBao,
    scope: VaultScope,
    name: &str,
    secret: &SecretWrite,
) -> Result<u64, OpenBaoError> {
    let body = json!({ "data": { "value": secret.value } });
    let written = bao
        .request(Method::POST, &scope.data(name), Some(&body))
        .await?
        .ok_or(OpenBaoError::Decode)?;
    let custom = json!({
        "custom_metadata": {
            "expires_at": secret.expires_at.clone().unwrap_or_default(),
            "used_by": secret.used_by.join(","),
        }
    });
    bao.request(Method::POST, &scope.metadata(name), Some(&custom))
        .await?;
    parse_version(&written)
}

pub async fn read_secret(
    bao: &OpenBao,
    scope: VaultScope,
    name: &str,
    version: Option<u64>,
) -> Result<SecretValue, OpenBaoError> {
    let suffix = version.map_or_else(String::new, |v| format!("?version={v}"));
    let response = bao
        .request(Method::GET, &format!("{}{suffix}", scope.data(name)), None)
        .await?
        .ok_or(OpenBaoError::Decode)?;
    let value = response
        .pointer("/data/data/value")
        .and_then(Value::as_str)
        .ok_or(OpenBaoError::Decode)?
        .to_string();
    Ok(SecretValue {
        value,
        version: response
            .pointer("/data/metadata/version")
            .and_then(Value::as_u64)
            .ok_or(OpenBaoError::Decode)?,
    })
}

pub async fn read_metadata(
    bao: &OpenBao,
    scope: VaultScope,
    name: &str,
) -> Result<Value, OpenBaoError> {
    bao.request(Method::GET, &scope.metadata(name), None)
        .await?
        .ok_or(OpenBaoError::Decode)
}

pub async fn list_names(bao: &OpenBao, scope: VaultScope) -> Result<Vec<String>, OpenBaoError> {
    match bao.request(Method::GET, &scope.listing(), None).await {
        Ok(Some(response)) => Ok(response
            .pointer("/data/keys")
            .and_then(Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()),
        Ok(None) | Err(OpenBaoError::Rejected { status: 404, .. }) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub async fn list_secrets(
    bao: &OpenBao,
    scope: VaultScope,
) -> Result<Vec<SecretSummary>, OpenBaoError> {
    let mut summaries = Vec::new();
    for name in list_names(bao, scope).await? {
        summaries.push(summarize(&name, &read_metadata(bao, scope, &name).await?)?);
    }
    Ok(summaries)
}

pub async fn soft_delete_secret(
    bao: &OpenBao,
    scope: VaultScope,
    name: &str,
) -> Result<(), OpenBaoError> {
    bao.request(Method::DELETE, &scope.data(name), None).await?;
    Ok(())
}

pub async fn restore_secret(
    bao: &OpenBao,
    scope: VaultScope,
    name: &str,
    version: u64,
) -> Result<(), OpenBaoError> {
    let body = json!({ "versions": [version] });
    bao.request(Method::POST, &scope.actions("undelete", name), Some(&body))
        .await?;
    Ok(())
}

pub async fn destroy_secret(
    bao: &OpenBao,
    scope: VaultScope,
    name: &str,
) -> Result<(), OpenBaoError> {
    delete_ignoring_missing(bao, &scope.metadata(name)).await
}

pub async fn purge_expired(
    bao: &OpenBao,
    scope: VaultScope,
    retention_days: i32,
) -> Result<usize, OpenBaoError> {
    let now = Utc::now();
    let mut purged = 0;
    for name in list_names(bao, scope).await? {
        let metadata = read_metadata(bao, scope, &name).await?;
        if deletion_overdue(&metadata, retention_days, now) == Some(true) {
            destroy_secret(bao, scope, &name).await?;
            purged += 1;
        }
    }
    Ok(purged)
}

async fn delete_ignoring_missing(bao: &OpenBao, path: &str) -> Result<(), OpenBaoError> {
    match bao.request(Method::DELETE, path, None).await {
        Ok(_) | Err(OpenBaoError::Rejected { status: 404, .. }) => Ok(()),
        Err(error) => Err(error),
    }
}

async fn remove_principal(
    bao: &OpenBao,
    scope: VaultScope,
    principal: &str,
) -> Result<(), OpenBaoError> {
    let role = scope.principal_role(principal);
    delete_ignoring_missing(bao, &format!("auth/approle/role/{role}")).await?;
    delete_ignoring_missing(bao, &format!("sys/policies/acl/{role}")).await
}

pub async fn sync_principal(
    bao: &OpenBao,
    scope: VaultScope,
    principal: &str,
    scopes: &[String],
) -> Result<(), OpenBaoError> {
    if scopes.is_empty() {
        return remove_principal(bao, scope, principal).await;
    }
    let role = scope.principal_role(principal);
    let policy = json!({ "policy": scope.principal_policy(scopes) });
    bao.request(
        Method::PUT,
        &format!("sys/policies/acl/{role}"),
        Some(&policy),
    )
    .await?;
    let definition = json!({
        "token_policies": [role],
        "token_ttl": "1h",
        "token_max_ttl": "24h",
        "secret_id_num_uses": 1,
        "secret_id_ttl": "10m",
    });
    bao.request(
        Method::POST,
        &format!("auth/approle/role/{role}"),
        Some(&definition),
    )
    .await?;
    Ok(())
}

pub async fn teardown(
    bao: &OpenBao,
    scope: VaultScope,
    principals: &[String],
) -> Result<(), OpenBaoError> {
    for name in list_names(bao, scope).await? {
        destroy_secret(bao, scope, &name).await?;
    }
    for principal in principals {
        remove_principal(bao, scope, principal).await?;
    }
    Ok(())
}

pub struct IssuedSecretId {
    pub role_id: String,
    pub wrapping_token: String,
}

pub async fn issue_secret_id(
    bao: &OpenBao,
    scope: VaultScope,
    principal: &str,
) -> Result<IssuedSecretId, OpenBaoError> {
    let role = scope.principal_role(principal);
    let identity = bao
        .request(
            Method::GET,
            &format!("auth/approle/role/{role}/role-id"),
            None,
        )
        .await?
        .ok_or(OpenBaoError::Decode)?;
    let wrapped = bao
        .request_wrapped(
            Method::POST,
            &format!("auth/approle/role/{role}/secret-id"),
            SECRET_ID_WRAP_TTL,
        )
        .await?
        .ok_or(OpenBaoError::Decode)?;
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_string);
    Ok(IssuedSecretId {
        role_id: text(identity.pointer("/data/role_id")).ok_or(OpenBaoError::Decode)?,
        wrapping_token: text(wrapped.pointer("/wrap_info/token")).ok_or(OpenBaoError::Decode)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> VaultScope {
        VaultScope {
            resource_group_id: Uuid::nil(),
            vault_id: Uuid::from_u128(1),
        }
    }

    #[test]
    fn names_reject_path_traversal_and_wildcards() {
        for name in ["", "a/b", "..", "a*", "a b", "a\n"] {
            assert!(validate_secret_name(name).is_err(), "{name}");
        }
        assert!(validate_secret_name("S3_SECRET-KEY1").is_ok());
        assert!(validate_secret_name(&"a".repeat(128)).is_err());
    }

    #[test]
    fn principal_policy_is_read_only_and_limited_to_requested_scopes() {
        let policy = scope().principal_policy(&["DATABASE_URL".into(), "SMTP_*".into()]);
        let prefix = format!("rg/{}/{}", Uuid::nil(), Uuid::from_u128(1));
        assert!(policy.contains(&format!("csfx-kv/data/{prefix}/DATABASE_URL")));
        assert!(policy.contains(&format!("csfx-kv/data/{prefix}/SMTP_*")));
        assert_eq!(policy.matches("path \"").count(), 2);
        assert!(!policy.contains("update") && !policy.contains("delete"));
    }

    #[test]
    fn scope_and_principal_validation_reject_injection() {
        for scope in ["", "a/b", "a\" {", "..", "a b"] {
            assert!(validate_scope(scope).is_err(), "{scope}");
        }
        assert!(validate_scope("SMTP_*").is_ok());
        assert!(validate_principal("app-stack.web").is_ok());
        assert!(validate_principal("a/b").is_err());
        assert!(validate_principal("").is_err());
    }

    #[test]
    fn scopes_match_exact_names_and_prefixes() {
        assert!(scope_matches("*", "ANY"));
        assert!(scope_matches("SMTP_*", "SMTP_HOST"));
        assert!(!scope_matches("SMTP_*", "WEBHOOK_KEY"));
        assert!(scope_matches("DATABASE_URL", "DATABASE_URL"));
        assert!(!scope_matches("DATABASE_URL", "DATABASE_URL2"));
    }

    #[test]
    fn paths_are_scoped_by_resource_group_and_vault() {
        let data = scope().data("TOKEN");
        assert!(data.starts_with("csfx-kv/data/rg/00000000-0000-0000-0000-000000000000/"));
        assert!(data.ends_with("/TOKEN"));
    }

    fn metadata(deletion_time: &str) -> Value {
        json!({ "data": {
            "current_version": 2,
            "updated_time": "2026-10-01T10:00:00Z",
            "custom_metadata": { "expires_at": "2027-01-01", "used_by": "api-gw,web" },
            "versions": { "2": { "deletion_time": deletion_time } },
        }})
    }

    #[test]
    fn summary_reads_custom_metadata_and_deletion_state() {
        let summary = summarize("TOKEN", &metadata("")).unwrap();
        assert_eq!(summary.version, 2);
        assert_eq!(summary.used_by, vec!["api-gw", "web"]);
        assert_eq!(summary.expires_at.as_deref(), Some("2027-01-01"));
        assert!(!summary.deleted);
        assert!(
            summarize("TOKEN", &metadata("2026-10-02T00:00:00Z"))
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn overdue_deletion_respects_retention() {
        let now = DateTime::parse_from_rfc3339("2026-11-15T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let deleted = metadata("2026-10-01T00:00:00Z");
        assert_eq!(deletion_overdue(&deleted, 30, now), Some(true));
        assert_eq!(deletion_overdue(&deleted, 60, now), Some(false));
        assert_eq!(deletion_overdue(&metadata(""), 30, now), None);
    }
}
