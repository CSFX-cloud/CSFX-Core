use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::{interval, Duration};

use crate::db::agents as agent_db;
use crate::db::events;
use crate::services::failover::FailoverService;

const POLL_INTERVAL_SECS: u64 = 30;
const SOFT_THRESHOLD_SECS: i64 = 120;
const HARD_THRESHOLD_SECS: i64 = 300;

pub async fn run(db: DatabaseConnection) {
    let failover = Arc::new(FailoverService::new(db.clone()));
    let mut tick = interval(Duration::from_secs(POLL_INTERVAL_SECS));

    loop {
        tick.tick().await;

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

        for agent in degraded {
            if let Err(e) = agent_db::set_agent_status(&db, agent.id, "Degraded").await {
                crate::log_error!(
                    "monitor",
                    &format!("Failed to mark agent degraded agent_id={} err={}", agent.id, e)
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

            if let Err(e) = events::insert(&db, Some(agent.id), "degraded", None, None).await {
                crate::log_error!(
                    "monitor",
                    &format!("Failed to insert degraded event agent_id={} err={}", agent.id, e)
                );
            }
        }

        for agent in offline {
            if let Err(e) = agent_db::set_agent_status(&db, agent.id, "Offline").await {
                crate::log_error!(
                    "monitor",
                    &format!("Failed to mark agent offline agent_id={} err={}", agent.id, e)
                );
                continue;
            }

            crate::log_warn!(
                "monitor",
                &format!("Agent offline agent_id={} triggering failover", agent.id)
            );

            let failover = failover.clone();
            let agent_id = agent.id;

            tokio::spawn(async move {
                if let Err(e) = failover.handle_failover(agent_id).await {
                    crate::log_error!(
                        "monitor",
                        &format!("Failover failed agent_id={} err={}", agent_id, e)
                    );
                }
            });
        }
    }
}
