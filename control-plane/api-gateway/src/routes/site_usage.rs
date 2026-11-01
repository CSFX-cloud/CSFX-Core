use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use entity::{site_deployments, site_usage_hourly, SiteDeployments, SiteUsageHourly};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::sites::load_site;
use crate::{auth::rbac::CanViewResourceGroups, site_service::SiteError, AppState};

const DEFAULT_DAYS: i64 = 7;
const MAX_DAYS: i64 = 90;
const MILLISECONDS_PER_MINUTE: f64 = 60_000.0;

#[derive(Deserialize)]
pub struct UsageQuery {
    days: Option<i64>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DailyUsage {
    pub date: NaiveDate,
    pub requests: i64,
    pub bytes_out: i64,
}

#[derive(Serialize)]
pub struct SiteUsage {
    days: i64,
    requests: i64,
    bytes_out: i64,
    build_minutes: f64,
    series: Vec<DailyUsage>,
}

pub fn clamp_days(requested: Option<i64>) -> i64 {
    requested.unwrap_or(DEFAULT_DAYS).clamp(1, MAX_DAYS)
}

pub fn aggregate(rows: &[site_usage_hourly::Model]) -> Vec<DailyUsage> {
    let mut by_day: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    for row in rows {
        let entry = by_day.entry(row.hour.date()).or_default();
        entry.0 += row.requests;
        entry.1 += row.bytes_out;
    }
    by_day
        .into_iter()
        .map(|(date, (requests, bytes_out))| DailyUsage {
            date,
            requests,
            bytes_out,
        })
        .collect()
}

pub fn build_minutes(durations: &[Option<i64>]) -> f64 {
    let total: i64 = durations.iter().flatten().sum();
    total as f64 / MILLISECONDS_PER_MINUTE
}

async fn usage_rows(
    state: &AppState,
    site_id: Uuid,
    since: NaiveDateTime,
) -> Result<Vec<site_usage_hourly::Model>, SiteError> {
    Ok(SiteUsageHourly::find()
        .filter(site_usage_hourly::Column::SiteId.eq(site_id))
        .filter(site_usage_hourly::Column::Hour.gte(since))
        .order_by_asc(site_usage_hourly::Column::Hour)
        .all(&state.db_conn)
        .await?)
}

async fn build_durations(
    state: &AppState,
    site_id: Uuid,
    since: NaiveDateTime,
) -> Result<Vec<Option<i64>>, SiteError> {
    Ok(SiteDeployments::find()
        .filter(site_deployments::Column::SiteId.eq(site_id))
        .filter(site_deployments::Column::CreatedAt.gte(since))
        .all(&state.db_conn)
        .await?
        .into_iter()
        .map(|deployment| deployment.duration_ms)
        .collect())
}

pub async fn get_usage(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<SiteUsage>, SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let days = clamp_days(query.days);
    let since = (Utc::now() - Duration::days(days)).naive_utc();
    let series = aggregate(&usage_rows(&state, site.id, since).await?);
    let durations = build_durations(&state, site.id, since).await?;
    Ok(Json(SiteUsage {
        days,
        requests: series.iter().map(|day| day.requests).sum(),
        bytes_out: series.iter().map(|day| day.bytes_out).sum(),
        build_minutes: build_minutes(&durations),
        series,
    }))
}

pub fn usage_routes() -> Router<AppState> {
    Router::new().route(
        "/resource-groups/{group_id}/sites/{site_id}/usage",
        get(get_usage),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(day: u32, hour: u32, requests: i64, bytes_out: i64) -> site_usage_hourly::Model {
        site_usage_hourly::Model {
            site_id: Uuid::nil(),
            hour: NaiveDate::from_ymd_opt(2026, 11, day)
                .unwrap()
                .and_hms_opt(hour, 0, 0)
                .unwrap(),
            requests,
            bytes_out,
        }
    }

    #[test]
    fn hourly_rows_are_summed_per_day_in_order() {
        let rows = [row(2, 3, 5, 50), row(1, 23, 1, 10), row(1, 4, 2, 20)];
        let series = aggregate(&rows);
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].requests, 3);
        assert_eq!(series[0].bytes_out, 30);
        assert_eq!(series[1].requests, 5);
    }

    #[test]
    fn the_window_is_clamped() {
        assert_eq!(clamp_days(None), 7);
        assert_eq!(clamp_days(Some(0)), 1);
        assert_eq!(clamp_days(Some(500)), 90);
    }

    #[test]
    fn build_time_ignores_unfinished_builds() {
        let minutes = build_minutes(&[Some(90_000), None, Some(30_000)]);
        assert!((minutes - 2.0).abs() < f64::EPSILON);
    }
}
