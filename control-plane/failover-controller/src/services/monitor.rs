use entity::entities::agents;
use sea_orm::DatabaseConnection;
use shared::leader::LeaderLease;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use uuid::Uuid;

use crate::db::agents as agent_db;
use crate::db::events;
use crate::services::eviction::{EvictionService, Pressure};
use crate::services::failover::FailoverService;
use crate::services::hardware_alerts;

const PRESSURE_RULES: [(&str, Pressure); 2] = [
    ("high_memory", Pressure::Memory),
    ("high_cpu", Pressure::Cpu),
];

const POLL_INTERVAL_SECS: u64 = 30;
const SOFT_THRESHOLD_SECS: i64 = 120;
const HARD_THRESHOLD_SECS: i64 = 300;

type InFlight = Arc<Mutex<HashSet<Uuid>>>;

pub async fn run(db: DatabaseConnection, leader: LeaderLease) {
    let failover = Arc::new(FailoverService::new(db.clone()));
    let eviction = EvictionService::new(db.clone());
    let in_flight = InFlight::default();
    let mut tick = interval(Duration::from_secs(POLL_INTERVAL_SECS));

    loop {
        tick.tick().await;

        if !leader.is_leader() {
            continue;
        }

        check_hardware_alerts(&db).await;
        relieve_pressure(&db, &eviction).await;

        let (degraded, offline) =
            match agent_db::get_stale_agents(&db, SOFT_THRESHOLD_SECS, HARD_THRESHOLD_SECS).await {
                Ok(result) => result,
                Err(e) => {
                    crate::log_error!(
                        "monitor",
                        &format!("Failed to query stale agents err={}", e)
                    );
                    continue;
                }
            };

        mark_degraded(&db, degraded).await;
        handle_offline(&db, &failover, &in_flight, offline).await;
    }
}

async fn check_hardware_alerts(db: &DatabaseConnection) {
    match agent_db::get_agents_for_alerting(db).await {
        Ok(agents) => {
            for agent in &agents {
                hardware_alerts::check_agent(db, agent).await;
            }
        }
        Err(e) => {
            crate::log_error!(
                "monitor",
                &format!("Failed to query agents for alerting err={}", e)
            );
        }
    }
}

async fn relieve_pressure(db: &DatabaseConnection, eviction: &EvictionService) {
    let agents = match agent_db::get_agents_for_alerting(db).await {
        Ok(agents) => agents,
        Err(e) => {
            crate::log_error!(
                "monitor",
                &format!("Failed to query agents for eviction err={}", e)
            );
            return;
        }
    };

    for agent in agents.iter().filter(|agent| agent.status == "Online") {
        for (event_type, pressure) in PRESSURE_RULES {
            if !has_open_event(db, agent.id, event_type).await {
                continue;
            }
            if let Err(e) = eviction.relieve(agent.id, pressure).await {
                crate::log_error!(
                    "monitor",
                    &format!("Eviction failed agent_id={} err={}", agent.id, e)
                );
            }
            break;
        }
    }
}

async fn has_open_event(db: &DatabaseConnection, agent_id: Uuid, event_type: &str) -> bool {
    match events::find_open(db, agent_id, event_type).await {
        Ok(event) => event.is_some(),
        Err(e) => {
            crate::log_error!(
                "monitor",
                &format!(
                    "Failed to query open event agent_id={} event_type={} err={}",
                    agent_id, event_type, e
                )
            );
            false
        }
    }
}

async fn mark_degraded(db: &DatabaseConnection, degraded: Vec<agents::Model>) {
    for agent in degraded {
        if let Err(e) = agent_db::set_agent_status(db, agent.id, "Degraded").await {
            crate::log_error!(
                "monitor",
                &format!(
                    "Failed to mark agent degraded agent_id={} err={}",
                    agent.id, e
                )
            );
            continue;
        }

        crate::log_warn!(
            "monitor",
            &format!(
                "Agent degraded agent_id={} last_heartbeat={:?}",
                agent.id, agent.last_heartbeat
            )
        );

        if let Err(e) = events::insert(db, Some(agent.id), "degraded", None, None).await {
            crate::log_error!(
                "monitor",
                &format!(
                    "Failed to insert degraded event agent_id={} err={}",
                    agent.id, e
                )
            );
        }
    }
}

async fn has_healthy_agent(db: &DatabaseConnection) -> bool {
    match agent_db::count_online(db).await {
        Ok(count) => count > 0,
        Err(e) => {
            crate::log_error!(
                "monitor",
                &format!("Failed to count online agents err={}", e)
            );
            false
        }
    }
}

async fn handle_offline(
    db: &DatabaseConnection,
    failover: &Arc<FailoverService>,
    in_flight: &InFlight,
    offline: Vec<agents::Model>,
) {
    if offline.is_empty() {
        return;
    }

    if !has_healthy_agent(db).await {
        crate::log_warn!(
            "monitor",
            &format!(
                "No healthy agent, skipping failover control_plane_isolation_suspected=true stale={}",
                offline.len()
            )
        );
        return;
    }

    for agent in offline {
        if agent.status != "Offline" && !mark_offline(db, &agent).await {
            continue;
        }
        start_failover(db, failover, in_flight, agent.id).await;
    }
}

async fn mark_offline(db: &DatabaseConnection, agent: &agents::Model) -> bool {
    if let Err(e) = agent_db::set_agent_status(db, agent.id, "Offline").await {
        crate::log_error!(
            "monitor",
            &format!(
                "Failed to mark agent offline agent_id={} err={}",
                agent.id, e
            )
        );
        return false;
    }

    crate::log_warn!("monitor", &format!("Agent offline agent_id={}", agent.id));
    true
}

async fn start_failover(
    db: &DatabaseConnection,
    failover: &Arc<FailoverService>,
    in_flight: &InFlight,
    agent_id: Uuid,
) {
    match agent_db::has_active_assignments(db, agent_id).await {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) => {
            crate::log_error!(
                "monitor",
                &format!(
                    "Failed to query assignments agent_id={} err={}",
                    agent_id, e
                )
            );
            return;
        }
    }

    if !in_flight.lock().await.insert(agent_id) {
        return;
    }

    crate::log_warn!(
        "monitor",
        &format!("Triggering failover agent_id={}", agent_id)
    );

    let failover = failover.clone();
    let in_flight = in_flight.clone();
    tokio::spawn(async move {
        if let Err(e) = failover.handle_failover(agent_id).await {
            crate::log_error!(
                "monitor",
                &format!("Failover failed agent_id={} err={}", agent_id, e)
            );
        }
        in_flight.lock().await.remove(&agent_id);
    });
}
