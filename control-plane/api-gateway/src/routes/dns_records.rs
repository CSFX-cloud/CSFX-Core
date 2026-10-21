use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get},
    Router,
};
use chrono::Utc;
use entity::entities::{dns_records, ingress_routes};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

use super::cluster::{error, internal, ApiError};
use super::ingress::{ensure_group, validate_hostname};
use crate::{
    auth::rbac::{CanManageResourceGroups, CanViewResourceGroups},
    dns_sync, AppState,
};

const DEFAULT_TTL: i32 = 30;
const MIN_TTL: i32 = 5;
const MAX_TTL: i32 = 3600;

#[derive(Debug, Deserialize)]
pub struct CreateRecordRequest {
    pub name: String,
    pub record_type: String,
    pub value: String,
    pub ttl: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct RecordResponse {
    pub id: Uuid,
    pub resource_group_id: Uuid,
    pub name: String,
    pub record_type: String,
    pub value: String,
    pub ttl: i32,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ZoneResponse {
    pub zone: String,
    pub servers: Vec<String>,
    pub console_host: String,
}

impl From<dns_records::Model> for RecordResponse {
    fn from(record: dns_records::Model) -> Self {
        Self {
            id: record.id,
            resource_group_id: record.resource_group_id,
            name: record.name,
            record_type: record.record_type,
            value: record.value,
            ttl: record.ttl,
            created_at: record.created_at.and_utc().to_rfc3339(),
        }
    }
}

pub struct ValidRecord {
    pub name: String,
    pub record_type: &'static str,
    pub value: String,
    pub ttl: i32,
}

pub fn validate_record(
    request: &CreateRecordRequest,
    zone: &str,
) -> Result<ValidRecord, &'static str> {
    let name = validate_hostname(&request.name)?;
    if !dns_sync::in_zone(&name, zone) {
        return Err("name must be inside the platform zone");
    }
    let ttl = request.ttl.unwrap_or(DEFAULT_TTL);
    if !(MIN_TTL..=MAX_TTL).contains(&ttl) {
        return Err("ttl must be between 5 and 3600 seconds");
    }
    let (record_type, value) = match request.record_type.to_ascii_uppercase().as_str() {
        "A" => (
            "A",
            request
                .value
                .trim()
                .parse::<Ipv4Addr>()
                .map_err(|_| "value must be an ipv4 address")?
                .to_string(),
        ),
        "CNAME" => ("CNAME", validate_hostname(&request.value)?),
        _ => return Err("record type must be A or CNAME"),
    };
    if value == name {
        return Err("record must not point at itself");
    }
    Ok(ValidRecord {
        name,
        record_type,
        value,
        ttl,
    })
}

pub async fn get_zone(CanViewResourceGroups(_claims): CanViewResourceGroups) -> Json<ZoneResponse> {
    let zone = dns_sync::zone_from_env();
    Json(ZoneResponse {
        console_host: dns_sync::platform_hostname(&zone),
        zone,
        servers: dns_sync::servers_from_env(),
    })
}

pub async fn list_records(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<Vec<RecordResponse>>, ApiError> {
    ensure_group(&state, group_id).await?;
    let records = dns_records::Entity::find()
        .filter(dns_records::Column::ResourceGroupId.eq(group_id))
        .order_by_asc(dns_records::Column::Name)
        .all(&state.db_conn)
        .await
        .map_err(internal)?;
    Ok(Json(records.into_iter().map(Into::into).collect()))
}

pub async fn create_record(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<CreateRecordRequest>,
) -> Result<(StatusCode, Json<RecordResponse>), ApiError> {
    ensure_group(&state, group_id).await?;
    let valid = validate_record(&request, &dns_sync::zone_from_env())
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    ensure_name_free(&state, &valid.name).await?;

    let record = dns_records::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group_id),
        name: Set(valid.name),
        record_type: Set(valid.record_type.to_string()),
        value: Set(valid.value),
        ttl: Set(valid.ttl),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await
    .map_err(internal)?;

    tracing::info!(user_id = %claims.user_id, record_id = %record.id, name = %record.name, "dns record created");
    Ok((StatusCode::CREATED, Json(record.into())))
}

async fn ensure_name_free(state: &AppState, name: &str) -> Result<(), ApiError> {
    let records = dns_records::Entity::find()
        .filter(dns_records::Column::Name.eq(name))
        .count(&state.db_conn)
        .await
        .map_err(internal)?;
    let routes = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::Hostname.eq(name))
        .count(&state.db_conn)
        .await
        .map_err(internal)?;
    if records + routes > 0 {
        return Err(error(StatusCode::CONFLICT, "name is already in use"));
    }
    Ok(())
}

pub async fn delete_record(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, record_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    ensure_group(&state, group_id).await?;
    let result = dns_records::Entity::delete_many()
        .filter(dns_records::Column::Id.eq(record_id))
        .filter(dns_records::Column::ResourceGroupId.eq(group_id))
        .exec(&state.db_conn)
        .await
        .map_err(internal)?;
    if result.rows_affected == 0 {
        return Err(error(StatusCode::NOT_FOUND, "dns record not found"));
    }

    tracing::info!(user_id = %claims.user_id, record_id = %record_id, "dns record deleted");
    Ok(StatusCode::NO_CONTENT)
}

pub fn dns_records_router() -> Router<AppState> {
    Router::new()
        .route("/dns/zone", get(get_zone))
        .route(
            "/resource-groups/{id}/dns-records",
            get(list_records).post(create_record),
        )
        .route(
            "/resource-groups/{id}/dns-records/{record_id}",
            delete(delete_record),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZONE: &str = "csfx.internal";

    fn request(
        name: &str,
        record_type: &str,
        value: &str,
        ttl: Option<i32>,
    ) -> CreateRecordRequest {
        CreateRecordRequest {
            name: name.to_string(),
            record_type: record_type.to_string(),
            value: value.to_string(),
            ttl,
        }
    }

    #[test]
    fn a_record_inside_the_zone_is_accepted() {
        let valid =
            validate_record(&request("DB.csfx.internal", "a", "10.0.0.9", None), ZONE).unwrap();
        assert_eq!(valid.name, "db.csfx.internal");
        assert_eq!(valid.record_type, "A");
        assert_eq!(valid.ttl, DEFAULT_TTL);
    }

    #[test]
    fn cname_target_is_validated_as_hostname() {
        let valid = validate_record(
            &request("www.csfx.internal", "CNAME", "Web.Example.com", Some(60)),
            ZONE,
        )
        .unwrap();
        assert_eq!(valid.value, "web.example.com");
        assert!(validate_record(
            &request("www.csfx.internal", "CNAME", "not valid", None),
            ZONE
        )
        .is_err());
    }

    #[test]
    fn invalid_records_are_rejected() {
        for bad in [
            request("db.example.com", "A", "10.0.0.9", None),
            request("db.csfx.internal", "A", "999.0.0.1", None),
            request("db.csfx.internal", "A", "10.0.0.9", Some(1)),
            request("db.csfx.internal", "A", "10.0.0.9", Some(7200)),
            request("db.csfx.internal", "TXT", "x", None),
            request("db.csfx.internal", "CNAME", "db.csfx.internal", None),
            request("csfx.internal", "A", "10.0.0.9", None),
        ] {
            assert!(validate_record(&bad, ZONE).is_err(), "{bad:?}");
        }
    }
}
