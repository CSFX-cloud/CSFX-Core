use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use chrono::{Duration, Utc};
use entity::{
    vault_audit, vault_grants, vault_reveal_approvals, vaults, VaultAudit, VaultGrants, Vaults,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageSecrets, CanRevealSecrets, CanViewSecrets},
    openbao::OpenBaoError,
    vault_service::{self as service, VaultError},
    vault_store::{self, SecretWrite},
    AppState,
};

const MAX_REASON_LENGTH: usize = 500;
const APPROVAL_VALIDITY_MINUTES: i64 = 10;
const CAPABILITIES: [&str; 4] = ["read_secrets", "sign_with_key", "reveal_secrets", "manage"];

pub struct ApiError(VaultError);

impl<E: Into<VaultError>> From<E> for ApiError {
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            VaultError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
            VaultError::NotFound => (StatusCode::NOT_FOUND, self.0.to_string()),
            VaultError::Sealed => (StatusCode::LOCKED, self.0.to_string()),
            VaultError::Invalid(reason) => (StatusCode::BAD_REQUEST, (*reason).to_string()),
            VaultError::Forbidden(reason) => (StatusCode::FORBIDDEN, (*reason).to_string()),
            VaultError::TimedOut => (StatusCode::GATEWAY_TIMEOUT, self.0.to_string()),
            VaultError::Store(OpenBaoError::Rejected { status: 404, .. }) => {
                (StatusCode::NOT_FOUND, "secret not found".to_string())
            }
            VaultError::Store(OpenBaoError::NotBootstrapped | OpenBaoError::Unreachable) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "secret store unavailable".to_string(),
            ),
            VaultError::Store(error) => {
                tracing::error!(error = %error, "secret store request failed");
                (StatusCode::BAD_GATEWAY, "secret store error".to_string())
            }
            VaultError::Database(error) => {
                tracing::error!(error = %error, "vault database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Serialize)]
pub struct VaultResponse {
    id: Uuid,
    resource_group_id: Uuid,
    name: String,
    sealed: bool,
    soft_delete_days: i32,
    created_at: chrono::NaiveDateTime,
}

impl From<vaults::Model> for VaultResponse {
    fn from(vault: vaults::Model) -> Self {
        Self {
            id: vault.id,
            resource_group_id: vault.resource_group_id,
            name: vault.name,
            sealed: vault.sealed,
            soft_delete_days: vault.soft_delete_days,
            created_at: vault.created_at,
        }
    }
}

#[derive(Serialize)]
struct VaultOverview {
    #[serde(flatten)]
    vault: VaultResponse,
    secret_count: usize,
    grant_count: u64,
    reads_24h: u64,
    denied_24h: u64,
}

#[derive(Deserialize)]
pub struct CreateVaultRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct SecretRequest {
    name: Option<String>,
    value: String,
    expires_at: Option<String>,
    #[serde(default)]
    used_by: Vec<String>,
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    version: u64,
}

#[derive(Deserialize)]
pub struct RevealRequest {
    reason: String,
    code: String,
    version: Option<u64>,
}

#[derive(Deserialize)]
pub struct GrantRequest {
    principal: String,
    capability: String,
    #[serde(default = "default_scope")]
    scope: String,
    #[serde(default)]
    requires_approval: bool,
}

#[derive(Deserialize)]
pub struct ApprovalRequest {
    user_id: Uuid,
    secret_name: String,
}

fn default_scope() -> String {
    "*".to_string()
}

fn validate_vault_name(name: &str) -> Result<(), VaultError> {
    let valid = !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(VaultError::Invalid(
            "vault name must be 1 to 63 lowercase letters, digits or hyphens",
        ))
    }
}

fn invalid(reason: &'static str) -> VaultError {
    VaultError::Invalid(reason)
}

async fn audit(
    state: &AppState,
    vault: &vaults::Model,
    actor: &str,
    action: &str,
    target: &str,
    result: &str,
) -> ApiResult<()> {
    service::record_audit(&state.db_conn, vault.id, actor, action, target, result).await?;
    Ok(())
}

pub async fn list_vaults(
    CanViewSecrets(_claims): CanViewSecrets,
    State(state): State<AppState>,
    Path(resource_group_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    service::load_group(&state, resource_group_id).await?;
    let found = Vaults::find()
        .filter(vaults::Column::ResourceGroupId.eq(resource_group_id))
        .order_by_asc(vaults::Column::Name)
        .all(&state.db_conn)
        .await?;
    Ok(Json(
        found
            .into_iter()
            .map(VaultResponse::from)
            .collect::<Vec<_>>(),
    ))
}

pub async fn create_vault(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path(resource_group_id): Path<Uuid>,
    Json(request): Json<CreateVaultRequest>,
) -> ApiResult<impl IntoResponse> {
    validate_vault_name(&request.name)?;
    service::load_group(&state, resource_group_id).await?;
    service::openbao(&state)?;
    let created = vaults::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(resource_group_id),
        name: Set(request.name),
        sealed: Set(false),
        soft_delete_days: Set(30),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await?;
    audit(
        &state,
        &created,
        &claims.username,
        "create vault",
        &created.name,
        "done",
    )
    .await?;
    Ok((StatusCode::CREATED, Json(VaultResponse::from(created))))
}

pub async fn get_vault(
    CanViewSecrets(_claims): CanViewSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    let secret_count =
        vault_store::list_names(service::openbao(&state)?, service::scope_of(&vault))
            .await?
            .len();
    let since = (Utc::now() - Duration::hours(24)).naive_utc();
    let recent = VaultAudit::find()
        .filter(vault_audit::Column::VaultId.eq(vault.id))
        .filter(vault_audit::Column::OccurredAt.gt(since));
    let reads_24h = recent
        .clone()
        .filter(vault_audit::Column::Action.is_in(["read", "reveal"]))
        .count(&state.db_conn)
        .await?;
    let denied_24h = recent
        .filter(vault_audit::Column::Result.starts_with("denied"))
        .count(&state.db_conn)
        .await?;
    let grant_count = VaultGrants::find()
        .filter(vault_grants::Column::VaultId.eq(vault.id))
        .count(&state.db_conn)
        .await?;
    Ok(Json(VaultOverview {
        vault: vault.into(),
        secret_count,
        grant_count,
        reads_24h,
        denied_24h,
    }))
}

pub async fn delete_vault(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    service::destroy_vault(&state, &vault).await?;
    audit(
        &state,
        &vault,
        &claims.username,
        "delete vault",
        &vault.name,
        "done",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_sealed(
    state: &AppState,
    claims_username: &str,
    ids: (Uuid, Uuid),
    sealed: bool,
) -> ApiResult<Json<VaultResponse>> {
    let vault = service::load_vault(state, ids.0, ids.1).await?;
    let mut active: vaults::ActiveModel = vault.into();
    active.sealed = Set(sealed);
    let updated = active.update(&state.db_conn).await?;
    let action = if sealed { "seal vault" } else { "unseal vault" };
    audit(
        state,
        &updated,
        claims_username,
        action,
        &updated.name,
        "done",
    )
    .await?;
    Ok(Json(updated.into()))
}

pub async fn seal_vault(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path(ids): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    set_sealed(&state, &claims.username, ids, true).await
}

pub async fn unseal_vault(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path(ids): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    set_sealed(&state, &claims.username, ids, false).await
}

pub async fn list_secrets(
    CanViewSecrets(_claims): CanViewSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    service::ensure_unsealed(&vault)?;
    let secrets =
        vault_store::list_secrets(service::openbao(&state)?, service::scope_of(&vault)).await?;
    Ok(Json(secrets))
}

async fn store_secret(
    state: &AppState,
    actor: &str,
    ids: (Uuid, Uuid),
    name: &str,
    request: SecretRequest,
) -> ApiResult<Json<serde_json::Value>> {
    vault_store::validate_secret_name(name).map_err(invalid)?;
    if request.value.is_empty() {
        return Err(invalid("value must not be empty").into());
    }
    let vault = service::load_vault(state, ids.0, ids.1).await?;
    service::ensure_unsealed(&vault)?;
    let secret = SecretWrite {
        value: request.value,
        expires_at: request.expires_at,
        used_by: request.used_by,
    };
    let version = vault_store::write_secret(
        service::openbao(state)?,
        service::scope_of(&vault),
        name,
        &secret,
    )
    .await?;
    audit(
        state,
        &vault,
        actor,
        &format!("update {name} -> v{version}"),
        name,
        "done",
    )
    .await?;
    Ok(Json(json!({ "name": name, "version": version })))
}

pub async fn create_secret(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path(ids): Path<(Uuid, Uuid)>,
    Json(mut request): Json<SecretRequest>,
) -> ApiResult<impl IntoResponse> {
    let name = request
        .name
        .take()
        .ok_or_else(|| invalid("name is required"))?;
    let stored = store_secret(&state, &claims.username, ids, &name, request).await?;
    Ok((StatusCode::CREATED, stored))
}

pub async fn update_secret(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id, name)): Path<(Uuid, Uuid, String)>,
    Json(request): Json<SecretRequest>,
) -> ApiResult<impl IntoResponse> {
    store_secret(
        &state,
        &claims.username,
        (resource_group_id, vault_id),
        &name,
        request,
    )
    .await
}

pub async fn delete_secret(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id, name)): Path<(Uuid, Uuid, String)>,
) -> ApiResult<impl IntoResponse> {
    vault_store::validate_secret_name(&name).map_err(invalid)?;
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    service::ensure_unsealed(&vault)?;
    vault_store::soft_delete_secret(service::openbao(&state)?, service::scope_of(&vault), &name)
        .await?;
    audit(
        &state,
        &vault,
        &claims.username,
        &format!("delete {name}"),
        &name,
        "done",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_secret(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id, name)): Path<(Uuid, Uuid, String)>,
    Json(request): Json<RestoreRequest>,
) -> ApiResult<impl IntoResponse> {
    vault_store::validate_secret_name(&name).map_err(invalid)?;
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    service::ensure_unsealed(&vault)?;
    let scope = service::scope_of(&vault);
    vault_store::restore_secret(service::openbao(&state)?, scope, &name, request.version).await?;
    audit(
        &state,
        &vault,
        &claims.username,
        &format!("restore {name} v{}", request.version),
        &name,
        "done",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn reveal_secret(
    CanRevealSecrets(claims): CanRevealSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id, name)): Path<(Uuid, Uuid, String)>,
    Json(request): Json<RevealRequest>,
) -> ApiResult<impl IntoResponse> {
    vault_store::validate_secret_name(&name).map_err(invalid)?;
    let reason = request.reason.trim();
    if reason.is_empty() || reason.len() > MAX_REASON_LENGTH {
        return Err(invalid("a reason of up to 500 characters is required").into());
    }
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    service::ensure_unsealed(&vault)?;
    let target = format!("reveal {name}");
    if let Some(denial) =
        service::authorize_reveal(&state, &vault, &claims, &name, &request.code).await?
    {
        audit(
            &state,
            &vault,
            &claims.username,
            &target,
            &name,
            &denial.audit_result(),
        )
        .await?;
        return Err(VaultError::Forbidden(denial.message()).into());
    }
    let secret = vault_store::read_secret(
        service::openbao(&state)?,
        service::scope_of(&vault),
        &name,
        request.version,
    )
    .await?;
    audit(
        &state,
        &vault,
        &claims.username,
        &format!("{target} ({reason})"),
        &name,
        "allowed",
    )
    .await?;
    Ok(Json(
        json!({ "name": name, "version": secret.version, "value": secret.value }),
    ))
}

pub async fn secret_versions(
    CanViewSecrets(_claims): CanViewSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id, name)): Path<(Uuid, Uuid, String)>,
) -> ApiResult<impl IntoResponse> {
    vault_store::validate_secret_name(&name).map_err(invalid)?;
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    service::ensure_unsealed(&vault)?;
    let metadata =
        vault_store::read_metadata(service::openbao(&state)?, service::scope_of(&vault), &name)
            .await?;
    Ok(Json(vault_store::version_list(&metadata)?))
}

pub async fn list_grants(
    CanViewSecrets(_claims): CanViewSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    let grants = VaultGrants::find()
        .filter(vault_grants::Column::VaultId.eq(vault.id))
        .order_by_asc(vault_grants::Column::CreatedAt)
        .all(&state.db_conn)
        .await?;
    Ok(Json(grants))
}

fn validate_grant(request: &GrantRequest) -> Result<(), VaultError> {
    vault_store::validate_principal(&request.principal).map_err(invalid)?;
    vault_store::validate_scope(&request.scope).map_err(invalid)?;
    if !CAPABILITIES.contains(&request.capability.as_str()) {
        return Err(invalid("unknown capability"));
    }
    Ok(())
}

pub async fn create_grant(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<GrantRequest>,
) -> ApiResult<impl IntoResponse> {
    validate_grant(&request)?;
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    let grant = vault_grants::ActiveModel {
        id: Set(Uuid::new_v4()),
        vault_id: Set(vault.id),
        principal: Set(request.principal.clone()),
        capability: Set(request.capability.clone()),
        scope: Set(request.scope.clone()),
        requires_approval: Set(request.requires_approval),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await?;
    service::sync_principal(&state, &vault, &grant.principal).await?;
    let target = format!(
        "{} {} on {}",
        grant.principal, grant.capability, grant.scope
    );
    audit(
        &state,
        &vault,
        &claims.username,
        "grant access",
        &target,
        "done",
    )
    .await?;
    Ok((StatusCode::CREATED, Json(grant)))
}

pub async fn delete_grant(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id, grant_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    let grant = VaultGrants::find_by_id(grant_id)
        .filter(vault_grants::Column::VaultId.eq(vault.id))
        .one(&state.db_conn)
        .await?
        .ok_or(VaultError::NotFound)?;
    VaultGrants::delete_by_id(grant.id)
        .exec(&state.db_conn)
        .await?;
    service::sync_principal(&state, &vault, &grant.principal).await?;
    let target = format!(
        "{} {} on {}",
        grant.principal, grant.capability, grant.scope
    );
    audit(
        &state,
        &vault,
        &claims.username,
        "revoke access",
        &target,
        "done",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn approve_reveal(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ApprovalRequest>,
) -> ApiResult<impl IntoResponse> {
    vault_store::validate_secret_name(&request.secret_name).map_err(invalid)?;
    if request.user_id == claims.user_id {
        return Err(VaultError::Forbidden("you cannot approve your own reveal").into());
    }
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    let now = Utc::now();
    let approval = vault_reveal_approvals::ActiveModel {
        id: Set(Uuid::new_v4()),
        vault_id: Set(vault.id),
        secret_name: Set(request.secret_name.clone()),
        approved_for: Set(request.user_id),
        approved_by: Set(claims.user_id),
        expires_at: Set((now + Duration::minutes(APPROVAL_VALIDITY_MINUTES)).naive_utc()),
        created_at: Set(now.naive_utc()),
    }
    .insert(&state.db_conn)
    .await?;
    audit(
        &state,
        &vault,
        &claims.username,
        "approve reveal",
        &request.secret_name,
        "done",
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "expires_at": approval.expires_at })),
    ))
}

pub async fn issue_secret_id(
    CanManageSecrets(claims): CanManageSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id, principal)): Path<(Uuid, Uuid, String)>,
) -> ApiResult<impl IntoResponse> {
    vault_store::validate_principal(&principal).map_err(invalid)?;
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    service::ensure_unsealed(&vault)?;
    if service::read_scopes(&state.db_conn, vault.id, &principal)
        .await?
        .is_empty()
    {
        return Err(VaultError::NotFound.into());
    }
    let issued = vault_store::issue_secret_id(
        service::openbao(&state)?,
        service::scope_of(&vault),
        &principal,
    )
    .await?;
    audit(
        &state,
        &vault,
        &claims.username,
        "issue secret id",
        &principal,
        "done",
    )
    .await?;
    Ok(Json(json!({
        "role_id": issued.role_id,
        "wrapping_token": issued.wrapping_token,
    })))
}

pub async fn list_audit(
    CanViewSecrets(_claims): CanViewSecrets,
    State(state): State<AppState>,
    Path((resource_group_id, vault_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    let vault = service::load_vault(&state, resource_group_id, vault_id).await?;
    Ok(Json(service::recent_audit(&state.db_conn, vault.id).await?))
}

pub fn vaults_routes() -> Router<AppState> {
    let vault = "/resource-groups/{resource_group_id}/vaults/{vault_id}";
    Router::new()
        .route(
            "/resource-groups/{resource_group_id}/vaults",
            get(list_vaults).post(create_vault),
        )
        .route(vault, get(get_vault).delete(delete_vault))
        .route(&format!("{vault}/seal"), post(seal_vault))
        .route(&format!("{vault}/unseal"), post(unseal_vault))
        .route(
            &format!("{vault}/secrets"),
            get(list_secrets).post(create_secret),
        )
        .route(
            &format!("{vault}/secrets/{{name}}"),
            axum::routing::put(update_secret).delete(delete_secret),
        )
        .route(
            &format!("{vault}/secrets/{{name}}/restore"),
            post(restore_secret),
        )
        .route(
            &format!("{vault}/secrets/{{name}}/reveal"),
            post(reveal_secret),
        )
        .route(
            &format!("{vault}/secrets/{{name}}/versions"),
            get(secret_versions),
        )
        .route(
            &format!("{vault}/grants"),
            get(list_grants).post(create_grant),
        )
        .route(
            &format!("{vault}/grants/{{grant_id}}"),
            delete(delete_grant),
        )
        .route(&format!("{vault}/approvals"), post(approve_reveal))
        .route(
            &format!("{vault}/principals/{{principal}}/secret-id"),
            post(issue_secret_id),
        )
        .route(&format!("{vault}/audit"), get(list_audit))
}
