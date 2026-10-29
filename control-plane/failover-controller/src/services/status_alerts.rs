use chrono::Utc;
use entity::entities::agents;
use sea_orm::DatabaseConnection;

use crate::db::events;

struct StatusRule {
    status: &'static str,
    event_type: &'static str,
    severity: &'static str,
}

const RULES: &[StatusRule] = &[
    StatusRule {
        status: "Degraded",
        event_type: "degraded",
        severity: "warning",
    },
    StatusRule {
        status: "Offline",
        event_type: "offline",
        severity: "critical",
    },
];

fn describe(agent: &agents::Model, rule: &StatusRule) -> String {
    let silence = agent
        .last_heartbeat
        .map(|ts| {
            format!(
                ", no heartbeat for {}s",
                (Utc::now().naive_utc() - ts).num_seconds()
            )
        })
        .unwrap_or_default();
    format!("Node {} is {}{}", agent.name, rule.event_type, silence)
}

pub async fn check_agent(db: &DatabaseConnection, agent: &agents::Model) {
    for rule in RULES {
        let existing = match events::find_open(db, agent.id, rule.event_type).await {
            Ok(v) => v,
            Err(e) => {
                crate::log_error!(
                    "status_alerts",
                    &format!(
                        "Failed to query open event agent_id={} event={} err={}",
                        agent.id, rule.event_type, e
                    )
                );
                continue;
            }
        };

        match (agent.status == rule.status, existing) {
            (true, None) => {
                let message = describe(agent, rule);
                if let Err(e) = events::insert_with_severity(
                    db,
                    Some(agent.id),
                    rule.event_type,
                    rule.severity,
                    Some(message.clone()),
                    None,
                    None,
                )
                .await
                {
                    crate::log_error!(
                        "status_alerts",
                        &format!(
                            "Failed to insert alert agent_id={} event={} err={}",
                            agent.id, rule.event_type, e
                        )
                    );
                    continue;
                }
                crate::log_warn!(
                    "status_alerts",
                    &format!(
                        "agent_id={} alert={} message={}",
                        agent.id, rule.event_type, message
                    )
                );
            }
            (false, Some(open_event)) => {
                if let Err(e) = events::resolve(db, open_event.id).await {
                    crate::log_error!(
                        "status_alerts",
                        &format!(
                            "Failed to resolve alert agent_id={} event={} err={}",
                            agent.id, rule.event_type, e
                        )
                    );
                    continue;
                }
                crate::log_info!(
                    "status_alerts",
                    &format!("agent_id={} alert={} resolved", agent.id, rule.event_type)
                );
            }
            _ => {}
        }
    }
}
