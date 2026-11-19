use crate::{
    auth::{jwt::Claims, middleware::authenticate},
    rbac_service::RbacService,
    AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

pub struct CanViewAgents(pub Claims);
pub struct CanViewWorkloads(pub Claims);
pub struct CanManageWorkloads(pub Claims);
pub struct CanViewVolumes(pub Claims);
pub struct CanManageVolumes(pub Claims);
pub struct CanViewNetworks(pub Claims);
pub struct CanManageNetworks(pub Claims);
pub struct CanManageSystem(pub Claims);
pub struct CanViewResourceGroups(pub Claims);
pub struct CanManageResourceGroups(pub Claims);
pub struct CanViewLogs(pub Claims);
pub struct CanManageLogs(pub Claims);
pub struct CanViewBuckets(pub Claims);
pub struct CanManageBuckets(pub Claims);
pub struct CanViewSecrets(pub Claims);
pub struct CanManageSecrets(pub Claims);
pub struct CanRevealSecrets(pub Claims);
pub struct CanManageRoles(pub Claims);
pub struct CanManageIdentityProviders(pub Claims);
pub struct CanUpdateOrganization(pub Claims);
pub struct CanViewUsers(pub Claims);
pub struct CanCreateUsers(pub Claims);
pub struct CanUpdateUsers(pub Claims);
pub struct CanDeleteUsers(pub Claims);
pub struct AuthenticatedUser(pub Claims);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        authenticate(parts, state).await.map(Self)
    }
}

async fn check(
    parts: &mut Parts,
    state: &AppState,
    resource: &str,
    action: &str,
) -> Result<Claims, StatusCode> {
    let claims = authenticate(parts, state).await?;
    let org_id = state.default_org_id;
    let rbac = RbacService::new(state.db_conn.clone());
    let allowed = rbac
        .has_permission(claims.user_id, org_id, resource, action)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(claims)
}

macro_rules! impl_extractor {
    ($ty:ty, $resource:expr, $action:expr) => {
        impl FromRequestParts<AppState> for $ty {
            type Rejection = StatusCode;

            async fn from_request_parts(
                parts: &mut Parts,
                state: &AppState,
            ) -> Result<Self, Self::Rejection> {
                check(parts, state, $resource, $action).await.map(Self)
            }
        }
    };
}

impl_extractor!(CanViewAgents, "agents", "view");
impl_extractor!(CanViewWorkloads, "workloads", "view");
impl_extractor!(CanManageWorkloads, "workloads", "manage");
impl_extractor!(CanViewVolumes, "volumes", "view");
impl_extractor!(CanManageVolumes, "volumes", "manage");
impl_extractor!(CanViewNetworks, "networks", "view");
impl_extractor!(CanManageNetworks, "networks", "manage");
impl_extractor!(CanManageSystem, "system", "manage");
impl_extractor!(CanViewResourceGroups, "resource_groups", "view");
impl_extractor!(CanManageResourceGroups, "resource_groups", "manage");
impl_extractor!(CanViewLogs, "logs", "view");
impl_extractor!(CanManageLogs, "logs", "manage");
impl_extractor!(CanViewBuckets, "buckets", "view");
impl_extractor!(CanManageBuckets, "buckets", "manage");
impl_extractor!(CanViewSecrets, "secrets", "view");
impl_extractor!(CanManageSecrets, "secrets", "manage");
impl_extractor!(CanRevealSecrets, "secrets", "reveal");
impl_extractor!(CanManageRoles, "roles", "manage");
impl_extractor!(CanManageIdentityProviders, "idp", "manage");
impl_extractor!(CanUpdateOrganization, "organization", "update");
impl_extractor!(CanViewUsers, "users", "view");
impl_extractor!(CanCreateUsers, "users", "create");
impl_extractor!(CanUpdateUsers, "users", "update");
impl_extractor!(CanDeleteUsers, "users", "delete");
