use crate::{
    openbao::OpenBaoError,
    site_service::{source_vault, SiteError},
    vault_service::{self, VaultError},
    vault_store,
    workload_secrets::{self, refs_in_env, source_of},
    AppState,
};
use entity::entities::workloads;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, DbErr, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use shared::env_secrets::{self, VaultRef};
use shared::leader::LeaderLease;
use shared::rollout::abandoned_attempts;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;
use tokio::time::{interval, sleep, MissedTickBehavior};
use uuid::Uuid;

const LEADER_KEY: &str = "/csfx/leaders/secret-rotation";
const CHECK_INTERVAL: Duration = Duration::from_secs(60);
const CONNECT_RETRY: Duration = Duration::from_secs(10);
const TOGGLE_VARIABLE: &str = "CSFX_SECRET_ROTATION_RESTART";
const STOPPED: &str = "stopped";

#[derive(Debug, PartialEq, Eq)]
pub enum Rotation {
    Unchanged,
    Record,
    Restart,
}

const MAX_ABANDONED_ROLLOUTS: u64 = 3;

pub fn decide(recorded: Option<&Value>, current: &Value) -> Rotation {
    match recorded {
        None => Rotation::Record,
        Some(previous) if abandoned_attempts(Some(previous)) >= MAX_ABANDONED_ROLLOUTS => {
            Rotation::Unchanged
        }
        Some(previous) if previous == current => Rotation::Unchanged,
        Some(_) => Rotation::Restart,
    }
}

pub fn enabled(value: Option<&str>) -> bool {
    !matches!(
        value.map(|raw| raw.trim().to_ascii_lowercase()).as_deref(),
        Some("false" | "0" | "no" | "off")
    )
}

pub fn spawn(state: AppState) {
    if !enabled(std::env::var(TOGGLE_VARIABLE).ok().as_deref()) {
        tracing::info!("secret rotation restarts are disabled");
        return;
    }
    tokio::spawn(run(state));
}

async fn run(state: AppState) {
    let leader = connect_with_retry().await;
    let mut tick = interval(CHECK_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        if !leader.is_leader() {
            continue;
        }
        if let Err(error) = check_all(&state, &leader).await {
            tracing::error!(error = %error, "secret rotation check failed");
        }
    }
}

async fn connect_with_retry() -> LeaderLease {
    loop {
        match LeaderLease::connect(LEADER_KEY).await {
            Ok(leader) => return leader,
            Err(error) => {
                tracing::warn!(error = %error, "secret rotation waiting for etcd");
                sleep(CONNECT_RETRY).await;
            }
        }
    }
}

type ReplicaKey = (Option<Uuid>, Option<Uuid>, Option<String>, Option<String>);

#[derive(Default)]
struct Run {
    versions: HashMap<String, u64>,
    restarted: HashSet<ReplicaKey>,
}

fn replica_key(workload: &workloads::Model) -> Option<ReplicaKey> {
    let grouped = workload.stack_id.is_some()
        || workload.service_name.is_some()
        || workload.spread_key.is_some();
    grouped.then(|| {
        (
            workload.resource_group_id,
            workload.stack_id,
            workload.service_name.clone(),
            workload.spread_key.clone(),
        )
    })
}

async fn check_all(state: &AppState, leader: &LeaderLease) -> Result<(), DbErr> {
    let rows = workloads::Entity::find()
        .filter(workloads::Column::EnvVars.is_not_null())
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::DesiredState.ne(STOPPED))
        .filter(workloads::Column::ReplacesWorkloadId.is_null())
        .filter(workloads::Column::RetireAfter.is_null())
        .all(&state.db_conn)
        .await?;
    let mut run = Run::default();
    for workload in rows {
        if !leader.is_leader() {
            break;
        }
        if let Err(error) = check_workload(state, &workload, &mut run).await {
            tracing::warn!(workload = %workload.id, error = %error, "secret rotation check skipped");
        }
    }
    Ok(())
}

fn references_of(workload: &workloads::Model) -> Result<Vec<VaultRef>, SiteError> {
    let Some(stored) = workload.env_vars.as_ref() else {
        return Ok(Vec::new());
    };
    let env = env_secrets::open(stored, &env_secrets::workload_env(workload.id))
        .map_err(|error| SiteError::Database(DbErr::Custom(error.to_string())))?;
    Ok(refs_in_env(&env))
}

async fn check_workload(
    state: &AppState,
    workload: &workloads::Model,
    run: &mut Run,
) -> Result<(), SiteError> {
    let references = references_of(workload)?;
    let Some(group_id) = workload
        .resource_group_id
        .filter(|_| !references.is_empty())
    else {
        return Ok(());
    };
    let current = current_versions(state, group_id, &references, &mut run.versions).await?;
    match decide(workload.secret_versions.as_ref(), &current) {
        Rotation::Unchanged => Ok(()),
        Rotation::Record => {
            claim(state, workload, &current).await?;
            Ok(())
        }
        Rotation::Restart => {
            restart_rotated(state, workload, group_id, &references, &current, run).await
        }
    }
}

async fn restart_rotated(
    state: &AppState,
    workload: &workloads::Model,
    group_id: Uuid,
    references: &[VaultRef],
    current: &Value,
    run: &mut Run,
) -> Result<(), SiteError> {
    if let Some(key) = replica_key(workload) {
        if !run.restarted.insert(key) {
            return Ok(());
        }
    }
    if !claim(state, workload, current).await? {
        return Ok(());
    }
    references
        .iter()
        .for_each(|reference| workload_secrets::invalidate(group_id, reference));
    if let Err(error) = restart(state, workload.id).await {
        release(state, workload).await?;
        return Err(error);
    }
    tracing::info!(workload = %workload.id, "workload restarted after secret rotation");
    Ok(())
}

async fn current_versions(
    state: &AppState,
    group_id: Uuid,
    references: &[VaultRef],
    seen: &mut HashMap<String, u64>,
) -> Result<Value, SiteError> {
    let mut versions = BTreeMap::new();
    for reference in references {
        let label = format!("{}/{}", reference.vault_id, reference.secret);
        let key = format!("{group_id}/{label}");
        let version = match seen.get(&key) {
            Some(version) => *version,
            None => {
                let version = read_version(state, group_id, reference).await?;
                seen.insert(key, version);
                version
            }
        };
        versions.insert(label, version);
    }
    Ok(json!(versions))
}

async fn read_version(
    state: &AppState,
    group_id: Uuid,
    reference: &VaultRef,
) -> Result<u64, SiteError> {
    let source = source_of(group_id, reference, String::new());
    let vault = source_vault(state, &source).await?;
    let bao = vault_service::openbao(state)?;
    let metadata =
        vault_store::read_metadata(bao, vault_service::scope_of(&vault), &reference.secret)
            .await
            .map_err(|error| match error {
                OpenBaoError::Rejected { status: 404, .. } => SiteError::NotFound,
                other => SiteError::Vault(VaultError::Store(other)),
            })?;
    vault_store::summarize(&reference.secret, &metadata)
        .map(|summary| summary.version)
        .map_err(|error| SiteError::Vault(VaultError::Store(error)))
}

async fn restart(state: &AppState, workload_id: Uuid) -> Result<(), SiteError> {
    let path = format!("/workloads/{workload_id}/restart");
    match state
        .service_client
        .forward_to_scheduler(reqwest::Method::POST, &path, None, None)
        .await
    {
        Ok((status, _)) if status.is_success() => Ok(()),
        _ => Err(SiteError::Upstream("scheduler rejected the restart")),
    }
}

async fn store(
    state: &AppState,
    workload: &workloads::Model,
    versions: Option<&Value>,
) -> Result<bool, SiteError> {
    let value = versions.map_or_else(
        || Expr::value(Option::<Value>::None),
        |recorded| Expr::value(recorded.clone()),
    );
    let applied = workloads::Entity::update_many()
        .col_expr(workloads::Column::SecretVersions, value)
        .filter(workloads::Column::Id.eq(workload.id))
        .filter(workloads::Column::Generation.eq(workload.generation))
        .exec(&state.db_conn)
        .await?;
    Ok(applied.rows_affected > 0)
}

async fn claim(
    state: &AppState,
    workload: &workloads::Model,
    versions: &Value,
) -> Result<bool, SiteError> {
    store(state, workload, Some(versions)).await
}

async fn release(state: &AppState, workload: &workloads::Model) -> Result<(), SiteError> {
    store(state, workload, workload.secret_versions.as_ref())
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toggle_accepts_common_spellings_of_off() {
        assert!(enabled(None));
        assert!(enabled(Some("true")));
        for off in ["false", "0", "no", "OFF", " off "] {
            assert!(!enabled(Some(off)));
        }
    }

    #[test]
    fn first_observation_only_records_a_baseline() {
        assert_eq!(decide(None, &json!({"v/A": 1})), Rotation::Record);
    }

    #[test]
    fn unchanged_versions_do_nothing() {
        let versions = json!({"v/A": 2});
        assert_eq!(decide(Some(&versions), &versions), Rotation::Unchanged);
    }

    #[test]
    fn a_new_secret_version_triggers_a_restart() {
        let before = json!({"v/A": 1});
        assert_eq!(decide(Some(&before), &json!({"v/A": 2})), Rotation::Restart);
    }
}
