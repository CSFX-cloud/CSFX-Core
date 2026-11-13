use chrono::{Duration as ChronoDuration, NaiveDateTime, Utc};
use entity::entities::{ingress_gateways, workloads};
use sea_orm::{ColumnTrait, DbErr, EntityTrait, QueryFilter};
use shared::leader::LeaderLease;
use std::time::Duration;
use tokio::time::{interval, sleep};
use uuid::Uuid;

use crate::routes::ingress_gateways::{
    confirm, remove_workload, STATUS_ACTIVE, STATUS_DELETING, STATUS_PENDING,
};
use crate::{zone_cert, AppState};

const LEADER_KEY: &str = "/csfx/leaders/gateway-reconciler";
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const CONNECT_RETRY: Duration = Duration::from_secs(10);
const PENDING_GRACE_SECONDS: i64 = 300;

#[derive(Debug, PartialEq)]
pub enum Action {
    Wait,
    Adopt(Uuid),
    Discard,
    Teardown,
}

pub fn decide(
    status: &str,
    created_at: NaiveDateTime,
    orphan: Option<Uuid>,
    now: NaiveDateTime,
) -> Action {
    if status == STATUS_DELETING {
        return Action::Teardown;
    }
    if status != STATUS_PENDING || now - created_at < ChronoDuration::seconds(PENDING_GRACE_SECONDS)
    {
        return Action::Wait;
    }
    orphan.map_or(Action::Discard, Action::Adopt)
}

async fn find_orphan(
    state: &AppState,
    gateway: &ingress_gateways::Model,
) -> Result<Option<Uuid>, DbErr> {
    let workload = workloads::Entity::find()
        .filter(workloads::Column::ResourceGroupId.eq(gateway.resource_group_id))
        .filter(workloads::Column::Name.eq(format!("gateway-{}", gateway.name)))
        .one(&state.db_conn)
        .await?;
    Ok(workload.map(|workload| workload.id))
}

async fn teardown(state: &AppState, gateway: &ingress_gateways::Model) -> anyhow::Result<()> {
    if let Some(workload_id) = gateway.workload_id {
        remove_workload(state, workload_id)
            .await
            .map_err(|(status, _)| anyhow::anyhow!("scheduler delete failed status={status}"))?;
    }
    ingress_gateways::Entity::delete_by_id(gateway.id)
        .exec(&state.db_conn)
        .await?;
    zone_cert::remove(&state.db_conn, gateway.id).await
}

async fn apply(state: &AppState, gateway: ingress_gateways::Model, action: Action) {
    let gateway_id = gateway.id;
    let outcome = match action {
        Action::Wait => return,
        Action::Adopt(workload_id) => confirm(state, gateway, workload_id)
            .await
            .map(|_| ())
            .map_err(|(status, _)| anyhow::anyhow!("confirm failed status={status}")),
        Action::Discard => ingress_gateways::Entity::delete_by_id(gateway_id)
            .exec(&state.db_conn)
            .await
            .map(|_| ())
            .map_err(anyhow::Error::from),
        Action::Teardown => teardown(state, &gateway).await,
    };
    match outcome {
        Ok(()) => tracing::info!(gateway_id = %gateway_id, "gateway reconciled"),
        Err(error) => {
            tracing::warn!(gateway_id = %gateway_id, error = %error, "gateway reconcile failed")
        }
    }
}

pub async fn reconcile_once(state: &AppState) -> Result<(), DbErr> {
    let stuck = ingress_gateways::Entity::find()
        .filter(ingress_gateways::Column::Status.ne(STATUS_ACTIVE))
        .all(&state.db_conn)
        .await?;
    let now = Utc::now().naive_utc();
    for gateway in stuck {
        let orphan = find_orphan(state, &gateway).await?;
        let action = decide(&gateway.status, gateway.created_at, orphan, now);
        apply(state, gateway, action).await;
    }
    Ok(())
}

pub fn spawn(state: AppState) {
    tokio::spawn(run(state));
}

async fn run(state: AppState) {
    let leader = connect_with_retry().await;
    let mut tick = interval(RECONCILE_INTERVAL);
    loop {
        tick.tick().await;
        if !leader.is_leader() {
            continue;
        }
        if let Err(error) = reconcile_once(&state).await {
            tracing::error!(error = %error, "gateway reconciliation failed");
        }
    }
}

async fn connect_with_retry() -> LeaderLease {
    loop {
        match LeaderLease::connect(LEADER_KEY).await {
            Ok(leader) => return leader,
            Err(error) => {
                tracing::warn!(error = %error, "gateway reconciler waiting for etcd");
                sleep(CONNECT_RETRY).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> NaiveDateTime {
        chrono::DateTime::from_timestamp(seconds, 0)
            .unwrap()
            .naive_utc()
    }

    #[test]
    fn fresh_pending_rows_are_left_alone() {
        assert_eq!(decide("pending", at(0), None, at(60)), Action::Wait);
    }

    #[test]
    fn stale_pending_rows_adopt_an_existing_workload() {
        let id = Uuid::from_u128(7);
        assert_eq!(
            decide("pending", at(0), Some(id), at(900)),
            Action::Adopt(id)
        );
    }

    #[test]
    fn stale_pending_rows_without_workload_are_discarded() {
        assert_eq!(decide("pending", at(0), None, at(900)), Action::Discard);
    }

    #[test]
    fn deleting_rows_are_torn_down_immediately() {
        assert_eq!(decide("deleting", at(0), None, at(1)), Action::Teardown);
    }

    #[test]
    fn active_rows_are_never_touched() {
        assert_eq!(decide("active", at(0), None, at(9000)), Action::Wait);
    }
}
