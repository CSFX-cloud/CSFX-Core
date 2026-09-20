use crate::{
    auth::jwt::{verify_jwt, Claims},
    rbac_service::RbacService,
    AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use chrono::Utc;
use entity::InvalidJwt;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

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
pub struct AuthenticatedUser(pub Claims);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        extract_claims(parts, state).await.map(Self)
    }
}

async fn extract_claims(parts: &mut Parts, state: &AppState) -> Result<Claims, StatusCode> {
    let token = parts
        .headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .or_else(|| {
            parts
                .headers
                .get("cookie")
                .and_then(|c| c.to_str().ok())
                .and_then(|cookies| {
                    cookies.split(';').find_map(|cookie| {
                        let mut p = cookie.trim().splitn(2, '=');
                        if p.next()? == "token" {
                            p.next()
                        } else {
                            None
                        }
                    })
                })
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let is_blacklisted = InvalidJwt::find()
        .filter(entity::invalid_jwt::Column::Token.eq(token))
        .one(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .is_some();

    if is_blacklisted {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token_data = verify_jwt(token).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if token_data.claims.exp < Utc::now().timestamp() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(token_data.claims)
}

fn get_default_org(state: &AppState) -> Result<Uuid, StatusCode> {
    state
        .default_org_id
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

async fn check(
    parts: &mut Parts,
    state: &AppState,
    resource: &str,
    action: &str,
) -> Result<Claims, StatusCode> {
    let claims = extract_claims(parts, state).await?;
    let org_id = get_default_org(state)?;
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
