use crate::{
    auth::jwt::{verify_jwt, Claims},
    rbac_service::RbacService,
    AppState,
};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use chrono::Utc;
use entity::{organization, InvalidJwt, Organization};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

pub struct RequirePermission {
    pub claims: Claims,
    pub organization_id: Uuid,
}

pub struct CanViewAgents(pub Claims);
pub struct CanManageAgents(pub Claims);
pub struct CanViewWorkloads(pub Claims);
pub struct CanManageWorkloads(pub Claims);
pub struct CanViewVolumes(pub Claims);
pub struct CanManageVolumes(pub Claims);
pub struct CanViewNetworks(pub Claims);
pub struct CanManageNetworks(pub Claims);

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
                        if p.next()? == "token" { p.next() } else { None }
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

async fn get_default_org(state: &AppState) -> Result<Uuid, StatusCode> {
    Organization::find()
        .filter(organization::Column::Name.eq("Default Organization"))
        .one(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(|o| o.id)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

async fn check(
    parts: &mut Parts,
    state: &AppState,
    resource: &str,
    action: &str,
) -> Result<Claims, StatusCode> {
    let claims = extract_claims(parts, state).await?;
    let org_id = get_default_org(state).await?;
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
        #[async_trait]
        impl FromRequestParts<AppState> for $ty {
            type Rejection = StatusCode;

            async fn from_request_parts(
                parts: &mut Parts,
                state: &AppState,
            ) -> Result<Self, Self::Rejection> {
                check(parts, state, $resource, $action)
                    .await
                    .map(Self)
            }
        }
    };
}

impl_extractor!(CanViewAgents, "agents", "view");
impl_extractor!(CanManageAgents, "agents", "manage");
impl_extractor!(CanViewWorkloads, "workloads", "view");
impl_extractor!(CanManageWorkloads, "workloads", "manage");
impl_extractor!(CanViewVolumes, "volumes", "view");
impl_extractor!(CanManageVolumes, "volumes", "manage");
impl_extractor!(CanViewNetworks, "networks", "view");
impl_extractor!(CanManageNetworks, "networks", "manage");
