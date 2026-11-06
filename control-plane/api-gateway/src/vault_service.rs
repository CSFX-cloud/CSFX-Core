use std::{collections::BTreeSet, time::Duration};

use chrono::Utc;
use entity::{
    resource_groups, vault_audit, vault_grants, vault_reveal_approvals, vaults, ResourceGroups,
    VaultAudit, VaultGrants, VaultRevealApprovals, Vaults,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter, QueryOrder, Set,
};
use thiserror::Error;
use tokio::time::{interval, timeout};
use uuid::Uuid;

use crate::{
    auth::jwt::Claims,
    auth_service::AuthService,
    openbao::{OpenBao, OpenBaoError},
    vault_store::{self, VaultScope},
    AppState,
};

const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(45);
const DELETION_RETRY_INTERVAL: Duration = Duration::from_secs(15);
const PURGE_INTERVAL: Duration = Duration::from_secs(3600);
const DELETING_STATUS: &str = "deleting";
const AUDIT_PAGE_SIZE: u64 = 200;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("vault service is not configured")]
    Unavailable,
    #[error("vault not found")]
    NotFound,
    #[error("vault is sealed")]
    Sealed,
    #[error("{0}")]
    Invalid(&'static str),
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("vault cleanup timed out")]
    TimedOut,
    #[error("secret store error: {0}")]
    Store(#[from] OpenBaoError),
    #[error("database error: {0}")]
    Database(#[from] DbErr),
}

pub fn openbao(state: &AppState) -> Result<&OpenBao, VaultError> {
    state.openbao.as_ref().ok_or(VaultError::Unavailable)
}

pub fn scope_of(vault: &vaults::Model) -> VaultScope {
    VaultScope {
        resource_group_id: vault.resource_group_id,
        vault_id: vault.id,
    }
}

pub async fn load_group(
    state: &AppState,
    resource_group_id: Uuid,
) -> Result<resource_groups::Model, VaultError> {
    let org_id = state.default_org_id.ok_or(VaultError::NotFound)?;
    ResourceGroups::find_by_id(resource_group_id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await?
        .ok_or(VaultError::NotFound)
}

pub async fn load_vault(
    state: &AppState,
    resource_group_id: Uuid,
    vault_id: Uuid,
) -> Result<vaults::Model, VaultError> {
    load_group(state, resource_group_id).await?;
    Vaults::find_by_id(vault_id)
        .filter(vaults::Column::ResourceGroupId.eq(resource_group_id))
        .one(&state.db_conn)
        .await?
        .ok_or(VaultError::NotFound)
}

pub fn ensure_unsealed(vault: &vaults::Model) -> Result<(), VaultError> {
    if vault.sealed {
        return Err(VaultError::Sealed);
    }
    Ok(())
}

pub async fn record_audit(
    db: &DbConn,
    vault_id: Uuid,
    actor: &str,
    action: &str,
    target: &str,
    result: &str,
) -> Result<(), DbErr> {
    vault_audit::ActiveModel {
        id: Set(Uuid::new_v4()),
        vault_id: Set(vault_id),
        occurred_at: Set(Utc::now().naive_utc()),
        actor: Set(actor.to_string()),
        action: Set(action.to_string()),
        target: Set(target.to_string()),
        source: Set("api-gateway".to_string()),
        result: Set(result.to_string()),
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn recent_audit(db: &DbConn, vault_id: Uuid) -> Result<Vec<vault_audit::Model>, DbErr> {
    use sea_orm::QuerySelect;
    VaultAudit::find()
        .filter(vault_audit::Column::VaultId.eq(vault_id))
        .order_by_desc(vault_audit::Column::OccurredAt)
        .limit(AUDIT_PAGE_SIZE)
        .all(db)
        .await
}

pub async fn read_scopes(
    db: &DbConn,
    vault_id: Uuid,
    principal: &str,
) -> Result<Vec<String>, DbErr> {
    Ok(VaultGrants::find()
        .filter(vault_grants::Column::VaultId.eq(vault_id))
        .filter(vault_grants::Column::Principal.eq(principal))
        .filter(vault_grants::Column::Capability.eq("read_secrets"))
        .all(db)
        .await?
        .into_iter()
        .map(|grant| grant.scope)
        .collect())
}

pub async fn sync_principal(
    state: &AppState,
    vault: &vaults::Model,
    principal: &str,
) -> Result<(), VaultError> {
    let scopes = read_scopes(&state.db_conn, vault.id, principal).await?;
    vault_store::sync_principal(openbao(state)?, scope_of(vault), principal, &scopes).await?;
    Ok(())
}

pub async fn reveal_requires_approval(
    db: &DbConn,
    vault_id: Uuid,
    email: &str,
    secret_name: &str,
) -> Result<bool, DbErr> {
    Ok(VaultGrants::find()
        .filter(vault_grants::Column::VaultId.eq(vault_id))
        .filter(vault_grants::Column::Principal.eq(email))
        .filter(vault_grants::Column::Capability.eq("reveal_secrets"))
        .filter(vault_grants::Column::RequiresApproval.eq(true))
        .all(db)
        .await?
        .iter()
        .any(|grant| vault_store::scope_matches(&grant.scope, secret_name)))
}

pub async fn has_active_approval(
    db: &DbConn,
    vault_id: Uuid,
    user_id: Uuid,
    secret_name: &str,
) -> Result<bool, DbErr> {
    Ok(VaultRevealApprovals::find()
        .filter(vault_reveal_approvals::Column::VaultId.eq(vault_id))
        .filter(vault_reveal_approvals::Column::ApprovedFor.eq(user_id))
        .filter(vault_reveal_approvals::Column::SecretName.eq(secret_name))
        .filter(vault_reveal_approvals::Column::ExpiresAt.gt(Utc::now().naive_utc()))
        .one(db)
        .await?
        .is_some())
}

async fn vault_principals(db: &DbConn, vault_id: Uuid) -> Result<Vec<String>, DbErr> {
    let principals: BTreeSet<String> = VaultGrants::find()
        .filter(vault_grants::Column::VaultId.eq(vault_id))
        .all(db)
        .await?
        .into_iter()
        .map(|grant| grant.principal)
        .collect();
    Ok(principals.into_iter().collect())
}

pub enum RevealDenial {
    TwoFactorRequired,
    InvalidCode,
    ApprovalRequired,
}

impl RevealDenial {
    pub fn message(&self) -> &'static str {
        match self {
            Self::TwoFactorRequired => {
                "two factor authentication must be enabled to reveal secrets"
            }
            Self::InvalidCode => "invalid two factor code",
            Self::ApprovalRequired => "approval required",
        }
    }

    pub fn audit_result(&self) -> String {
        match self {
            Self::TwoFactorRequired => "denied - no 2fa".to_string(),
            Self::InvalidCode => "denied - invalid 2fa code".to_string(),
            Self::ApprovalRequired => "denied - no approval".to_string(),
        }
    }
}

pub async fn authorize_reveal(
    state: &AppState,
    vault: &vaults::Model,
    claims: &Claims,
    secret_name: &str,
    code: &str,
) -> Result<Option<RevealDenial>, VaultError> {
    let auth = AuthService::new(state.db_conn.clone());
    let user = auth
        .get_user_by_id(claims.user_id)
        .await
        .map_err(|_| VaultError::NotFound)?;
    if !user.two_factor_enabled {
        return Ok(Some(RevealDenial::TwoFactorRequired));
    }
    let valid = auth
        .verify_2fa_code(&user, code)
        .map_err(|_| VaultError::Forbidden("two factor verification failed"))?;
    if !valid {
        return Ok(Some(RevealDenial::InvalidCode));
    }
    let needs_approval =
        reveal_requires_approval(&state.db_conn, vault.id, &claims.username, secret_name).await?;
    if needs_approval
        && !has_active_approval(&state.db_conn, vault.id, claims.user_id, secret_name).await?
    {
        return Ok(Some(RevealDenial::ApprovalRequired));
    }
    Ok(None)
}

pub async fn destroy_vault(state: &AppState, vault: &vaults::Model) -> Result<(), VaultError> {
    let principals = vault_principals(&state.db_conn, vault.id).await?;
    vault_store::teardown(openbao(state)?, scope_of(vault), &principals).await?;
    Vaults::delete_by_id(vault.id).exec(&state.db_conn).await?;
    Ok(())
}

async fn teardown_vaults(state: &AppState, resource_group_id: Uuid) -> Result<(), VaultError> {
    let group_vaults = Vaults::find()
        .filter(vaults::Column::ResourceGroupId.eq(resource_group_id))
        .all(&state.db_conn)
        .await?;
    if group_vaults.is_empty() {
        return Ok(());
    }
    let bao = openbao(state)?;
    for vault in &group_vaults {
        let principals = vault_principals(&state.db_conn, vault.id).await?;
        vault_store::teardown(bao, scope_of(vault), &principals).await?;
        tracing::info!(vault_id = %vault.id, resource_group_id = %resource_group_id, "vault secrets destroyed");
    }
    Ok(())
}

pub async fn teardown_resource_group(
    state: &AppState,
    resource_group_id: Uuid,
) -> Result<(), VaultError> {
    timeout(TEARDOWN_TIMEOUT, teardown_vaults(state, resource_group_id))
        .await
        .map_err(|_| VaultError::TimedOut)?
}

async fn finish_pending_deletions(state: &AppState) {
    let pending = match ResourceGroups::find()
        .filter(resource_groups::Column::Status.eq(DELETING_STATUS))
        .all(&state.db_conn)
        .await
    {
        Ok(groups) => groups,
        Err(error) => {
            tracing::error!(error = %error, "failed to list resource groups pending deletion");
            return;
        }
    };
    for group in pending {
        if let Err(error) = teardown_resource_group(state, group.id).await {
            tracing::warn!(resource_group_id = %group.id, error = %error, "vault cleanup retry failed");
            continue;
        }
        if let Err(error) = ResourceGroups::delete_by_id(group.id)
            .exec(&state.db_conn)
            .await
        {
            tracing::error!(resource_group_id = %group.id, error = %error, "failed to delete resource group after vault cleanup");
        }
    }
}

pub fn spawn_deletion_job(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(DELETION_RETRY_INTERVAL);
        loop {
            ticker.tick().await;
            finish_pending_deletions(&state).await;
        }
    });
}

async fn purge_all(state: &AppState) {
    let Some(bao) = state.openbao.as_ref() else {
        return;
    };
    let all_vaults = match Vaults::find().all(&state.db_conn).await {
        Ok(found) => found,
        Err(error) => {
            tracing::error!(error = %error, "failed to list vaults for purge");
            return;
        }
    };
    for vault in all_vaults {
        match vault_store::purge_expired(bao, scope_of(&vault), vault.soft_delete_days).await {
            Ok(purged) if purged > 0 => {
                tracing::info!(vault_id = %vault.id, purged = purged, "expired soft deleted secrets destroyed");
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(vault_id = %vault.id, error = %error, "secret purge failed");
            }
        }
    }
}

pub fn spawn_purge_job(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(PURGE_INTERVAL);
        loop {
            ticker.tick().await;
            purge_all(&state).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault(sealed: bool) -> vaults::Model {
        vaults::Model {
            id: Uuid::new_v4(),
            resource_group_id: Uuid::new_v4(),
            name: "production".to_string(),
            sealed,
            soft_delete_days: 7,
            created_at: Utc::now().naive_utc(),
        }
    }

    #[test]
    fn scope_carries_the_group_and_vault_ids() {
        let vault = vault(false);
        let scope = scope_of(&vault);
        assert_eq!(scope.resource_group_id, vault.resource_group_id);
        assert_eq!(scope.vault_id, vault.id);
    }

    #[test]
    fn sealed_vaults_are_rejected() {
        assert!(ensure_unsealed(&vault(false)).is_ok());
        assert!(matches!(
            ensure_unsealed(&vault(true)),
            Err(VaultError::Sealed)
        ));
    }

    #[test]
    fn reveal_denials_expose_message_and_audit_result() {
        let denials = [
            (RevealDenial::TwoFactorRequired, "denied - no 2fa"),
            (RevealDenial::InvalidCode, "denied - invalid 2fa code"),
            (RevealDenial::ApprovalRequired, "denied - no approval"),
        ];
        for (denial, audit) in denials {
            assert_eq!(denial.audit_result(), audit);
            assert!(!denial.message().is_empty());
        }
        assert_eq!(
            RevealDenial::ApprovalRequired.message(),
            "approval required"
        );
    }

    #[test]
    fn vault_errors_render_readable_messages() {
        assert_eq!(
            VaultError::Unavailable.to_string(),
            "vault service is not configured"
        );
        assert_eq!(VaultError::NotFound.to_string(), "vault not found");
        assert_eq!(VaultError::Sealed.to_string(), "vault is sealed");
        assert_eq!(VaultError::Invalid("bad name").to_string(), "bad name");
        assert_eq!(VaultError::Forbidden("no access").to_string(), "no access");
        assert_eq!(VaultError::TimedOut.to_string(), "vault cleanup timed out");
        assert!(VaultError::Database(DbErr::Custom("boom".to_string()))
            .to_string()
            .starts_with("database error:"));
    }
}
