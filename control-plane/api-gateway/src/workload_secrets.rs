use crate::{
    auth::jwt::Claims,
    openbao::OpenBaoError,
    routes::sites::permitted,
    site_service::{fetch_secret, source_vault, SecretSource, SiteError},
    ttl_cache::TtlCache,
    vault_service::{self, VaultError},
    vault_store, AppState,
};
use entity::entities::workloads;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Map, Value};
use shared::env_secrets::{parse_vault_ref, VaultRef};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use uuid::Uuid;

const CACHE_TTL: Duration = Duration::from_secs(120);
const VAULT_SCHEME: &str = "vault://";

static LAST_RESOLVED: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static RESOLVED: LazyLock<TtlCache<String>> = LazyLock::new(|| TtlCache::new(CACHE_TTL));

fn references_in(entries: &Map<String, Value>) -> Vec<VaultRef> {
    entries
        .values()
        .filter_map(|value| value.as_str().and_then(parse_vault_ref))
        .collect()
}

pub(crate) fn source_of(group_id: Uuid, reference: &VaultRef, actor: String) -> SecretSource {
    SecretSource {
        resource_group_id: group_id,
        vault_id: Some(reference.vault_id),
        actor,
    }
}

pub fn reject_malformed_refs(env: &Value) -> Result<(), SiteError> {
    let Some(entries) = env.as_object() else {
        return Ok(());
    };
    let malformed = entries
        .values()
        .filter_map(Value::as_str)
        .find(|value| value.starts_with(VAULT_SCHEME) && parse_vault_ref(value).is_none());
    match malformed {
        Some(value) => Err(SiteError::Invalid(format!(
            "invalid vault reference {value}"
        ))),
        None => Ok(()),
    }
}

pub fn refs_in_env(env: &Value) -> Vec<VaultRef> {
    env.as_object().map(references_in).unwrap_or_default()
}

pub fn refs_in_text(text: &str) -> Result<Vec<VaultRef>, SiteError> {
    text.match_indices(VAULT_SCHEME)
        .map(|(start, _)| {
            let token = text[start..]
                .split(|c: char| c.is_whitespace() || "\"',]}".contains(c))
                .next()
                .unwrap_or_default();
            parse_vault_ref(token)
                .ok_or_else(|| SiteError::Invalid(format!("invalid vault reference {token}")))
        })
        .collect()
}

pub fn refs_in_yaml(text: &str) -> Result<Vec<VaultRef>, SiteError> {
    let document: serde_yaml::Value = serde_yaml::from_str(text)
        .map_err(|_| SiteError::Invalid("compose_yaml is not valid yaml".to_string()))?;
    let mut found = Vec::new();
    collect_yaml_refs(&document, &mut found)?;
    Ok(found)
}

fn collect_yaml_refs(
    value: &serde_yaml::Value,
    found: &mut Vec<VaultRef>,
) -> Result<(), SiteError> {
    match value {
        serde_yaml::Value::String(text) => found.extend(refs_in_text(text)?),
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .try_for_each(|item| collect_yaml_refs(item, found))?,
        serde_yaml::Value::Mapping(entries) => entries.iter().try_for_each(|(key, item)| {
            collect_yaml_refs(key, found)?;
            collect_yaml_refs(item, found)
        })?,
        serde_yaml::Value::Tagged(tagged) => collect_yaml_refs(&tagged.value, found)?,
        _ => {}
    }
    Ok(())
}

pub async fn authorize(
    state: &AppState,
    claims: &Claims,
    group_id: Option<Uuid>,
    references: &[VaultRef],
    already_bound: &[VaultRef],
    spec_change: bool,
) -> Result<(), SiteError> {
    if references.is_empty() && already_bound.is_empty() {
        return Ok(());
    }
    let group_id = group_id
        .ok_or_else(|| SiteError::Invalid("vault references need a resource group".to_string()))?;
    if !permitted(state, claims, "secrets", "reveal").await {
        return Err(SiteError::Forbidden);
    }
    for reference in references.iter().filter(|r| !already_bound.contains(r)) {
        let source = source_of(group_id, reference, String::new());
        let vault = source_vault(state, &source).await?;
        ensure_bindable(state, claims, &vault, &reference.secret).await?;
    }
    if spec_change {
        for reference in already_bound {
            ensure_not_gated(state, claims, group_id, reference).await?;
        }
    }
    Ok(())
}

async fn ensure_not_gated(
    state: &AppState,
    claims: &Claims,
    group_id: Uuid,
    reference: &VaultRef,
) -> Result<(), SiteError> {
    let vault = entity::Vaults::find_by_id(reference.vault_id)
        .filter(entity::vaults::Column::ResourceGroupId.eq(group_id))
        .one(&state.db_conn)
        .await?;
    match vault {
        Some(vault) => ensure_unrestricted(state, claims, vault.id, &reference.secret).await,
        None => Ok(()),
    }
}

async fn ensure_unrestricted(
    state: &AppState,
    claims: &Claims,
    vault_id: Uuid,
    secret: &str,
) -> Result<(), SiteError> {
    let gated =
        vault_service::reveal_requires_approval(&state.db_conn, vault_id, &claims.username, secret)
            .await?;
    if gated {
        return Err(SiteError::Invalid(format!(
            "secret {secret} requires approval and cannot be bound to a workload"
        )));
    }
    Ok(())
}

async fn ensure_bindable(
    state: &AppState,
    claims: &Claims,
    vault: &entity::vaults::Model,
    secret: &str,
) -> Result<(), SiteError> {
    ensure_unrestricted(state, claims, vault.id, secret).await?;
    let bao = vault_service::openbao(state)?;
    vault_store::read_metadata(bao, vault_service::scope_of(vault), secret)
        .await
        .map(|_| ())
        .map_err(|error| match error {
            OpenBaoError::Rejected { status: 404, .. } => {
                SiteError::Invalid(format!("secret {secret} does not exist in the vault"))
            }
            other => SiteError::Vault(VaultError::Store(other)),
        })
}

fn cache_key(group_id: Uuid, reference: &VaultRef) -> String {
    format!("{group_id}/{}/{}", reference.vault_id, reference.secret)
}

pub fn invalidate(group_id: Uuid, reference: &VaultRef) {
    RESOLVED.remove(&cache_key(group_id, reference));
}

pub async fn resolve_env(
    state: &AppState,
    workload: &workloads::Model,
) -> Result<Option<Value>, SiteError> {
    let Some(Value::Object(entries)) = &workload.env_vars else {
        return Ok(workload.env_vars.clone());
    };
    if references_in(entries).is_empty() {
        return Ok(workload.env_vars.clone());
    }
    let group_id = workload
        .resource_group_id
        .ok_or_else(|| SiteError::Invalid("vault references need a resource group".to_string()))?;
    let mut resolved = entries.clone();
    for value in resolved.values_mut() {
        let Some(reference) = value.as_str().and_then(parse_vault_ref) else {
            continue;
        };
        let fresh = workload.restart_requested
            || (workload.replaces_workload_id.is_some() && workload.status != "running");
        *value = Value::String(resolve_one(state, workload.id, group_id, &reference, fresh).await?);
    }
    Ok(Some(Value::Object(resolved)))
}

async fn resolve_one(
    state: &AppState,
    workload_id: Uuid,
    group_id: Uuid,
    reference: &VaultRef,
    fresh: bool,
) -> Result<String, SiteError> {
    let key = cache_key(group_id, reference);
    if let Some(cached) = RESOLVED.get(&key).filter(|_| !fresh) {
        return Ok(cached);
    }
    let source = source_of(group_id, reference, format!("workload:{workload_id}"));
    match fetch_secret(state, &source, &reference.secret).await {
        Ok(value) => {
            RESOLVED.insert(&key, value.clone());
            last_resolved().insert(key, value.clone());
            Ok(value)
        }
        Err(error) if is_transient(&error) => {
            last_resolved().get(&key).cloned().ok_or(error).inspect(|_| {
                tracing::warn!(workload = %workload_id, secret = %reference.secret, "vault unavailable, using last resolved value");
            })
        }
        Err(error) => {
            last_resolved().remove(&key);
            Err(error)
        }
    }
}

fn is_transient(error: &SiteError) -> bool {
    matches!(
        error,
        SiteError::Database(_)
            | SiteError::Vault(VaultError::Unavailable)
            | SiteError::Vault(VaultError::Store(
                OpenBaoError::Unreachable | OpenBaoError::NotBootstrapped
            ))
    ) || matches!(
        error,
        SiteError::Vault(VaultError::Store(OpenBaoError::Rejected { status, .. })) if *status >= 500
    )
}

fn last_resolved() -> std::sync::MutexGuard<'static, HashMap<String, String>> {
    LAST_RESOLVED.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_vault_references_are_collected() {
        let id = Uuid::new_v4();
        let env = json!({
            "PORT": "8080",
            "DB_PASSWORD": format!("vault://{id}/DB_PASSWORD"),
            "BROKEN": "vault://nope"
        });
        let references = references_in(env.as_object().unwrap());
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].vault_id, id);
        assert_eq!(references[0].secret, "DB_PASSWORD");
    }

    #[test]
    fn compose_text_references_are_found_and_malformed_ones_rejected() {
        let id = Uuid::new_v4();
        let yaml = format!(
            "environment:\n  - A=vault://{id}/DB_PASSWORD\n  B: \"vault://{id}/API-KEY\"\n"
        );
        let found = refs_in_text(&yaml).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].secret, "API-KEY");
        assert!(refs_in_text(&format!("A=vault://{id}/../../x/Y")).is_err());
        assert!(refs_in_text("plain: value").unwrap().is_empty());
    }

    #[test]
    fn yaml_escapes_cannot_hide_references() {
        let id = Uuid::new_v4();
        let yaml = format!(
            "services:\n  web:\n    environment:\n      B: \"vault:\\x2f/{id}/DB_PASSWORD\"\n"
        );
        let found = refs_in_yaml(&yaml).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].secret, "DB_PASSWORD");
        assert!(refs_in_yaml("services: [").is_err());
        assert!(refs_in_yaml("services:\n  web:\n    image: nginx\n")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn malformed_references_in_env_are_rejected() {
        let id = Uuid::new_v4();
        let ok = json!({ "A": format!("vault://{id}/KEY"), "B": "plain" });
        assert!(reject_malformed_refs(&ok).is_ok());
        let traversal = json!({ "A": format!("vault://{id}/../../x/Y") });
        assert!(reject_malformed_refs(&traversal).is_err());
        assert!(reject_malformed_refs(&json!({ "A": "vault://nope" })).is_err());
    }
}
