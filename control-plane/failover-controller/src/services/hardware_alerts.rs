use chrono::Utc;
use entity::entities::{agent_metrics, agents};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::db::events;

const SUSTAINED_WINDOW_SECS: i64 = 300;
const CPU_THRESHOLD_PERCENT: f32 = 90.0;
const MEMORY_THRESHOLD_PERCENT: f32 = 90.0;
const DISK_THRESHOLD_PERCENT: f32 = 85.0;

struct ThresholdRule {
    event_type: &'static str,
    severity: &'static str,
    threshold: f32,
    label: &'static str,
}

const RULES: &[ThresholdRule] = &[
    ThresholdRule {
        event_type: "high_cpu",
        severity: "warning",
        threshold: CPU_THRESHOLD_PERCENT,
        label: "CPU usage",
    },
    ThresholdRule {
        event_type: "high_memory",
        severity: "warning",
        threshold: MEMORY_THRESHOLD_PERCENT,
        label: "Memory usage",
    },
    ThresholdRule {
        event_type: "high_disk",
        severity: "warning",
        threshold: DISK_THRESHOLD_PERCENT,
        label: "Disk usage",
    },
];

fn sample_value(sample: &agent_metrics::Model, event_type: &str) -> Option<f32> {
    match event_type {
        "high_cpu" => sample.cpu_usage_percent,
        "high_memory" => sample.memory_usage_percent,
        "high_disk" => sample.disk_usage_percent,
        _ => None,
    }
}

async fn is_sustained_above(
    db: &DatabaseConnection,
    agent_id: Uuid,
    event_type: &str,
    threshold: f32,
) -> anyhow::Result<bool> {
    let cutoff = Utc::now().naive_utc() - chrono::Duration::seconds(SUSTAINED_WINDOW_SECS);

    let samples = agent_metrics::Entity::find()
        .filter(agent_metrics::Column::AgentId.eq(agent_id))
        .filter(agent_metrics::Column::Timestamp.gt(cutoff))
        .order_by_desc(agent_metrics::Column::Timestamp)
        .limit(20)
        .all(db)
        .await?;

    if samples.is_empty() {
        return Ok(false);
    }

    Ok(samples
        .iter()
        .all(|s| sample_value(s, event_type).is_some_and(|v| v > threshold)))
}

pub async fn check_agent(db: &DatabaseConnection, agent: &agents::Model) {
    for rule in RULES {
        let sustained =
            match is_sustained_above(db, agent.id, rule.event_type, rule.threshold).await {
                Ok(v) => v,
                Err(e) => {
                    crate::log_error!(
                        "hardware_alerts",
                        &format!(
                            "Failed to evaluate rule agent_id={} rule={} err={}",
                            agent.id, rule.event_type, e
                        )
                    );
                    continue;
                }
            };

        let existing = match events::find_open(db, agent.id, rule.event_type).await {
            Ok(v) => v,
            Err(e) => {
                crate::log_error!(
                    "hardware_alerts",
                    &format!(
                        "Failed to query open event agent_id={} rule={} err={}",
                        agent.id, rule.event_type, e
                    )
                );
                continue;
            }
        };

        match (sustained, existing) {
            (true, None) => {
                let message = format!(
                    "{} above {:.0}% for {}m",
                    rule.label,
                    rule.threshold,
                    SUSTAINED_WINDOW_SECS / 60
                );
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
                        "hardware_alerts",
                        &format!(
                            "Failed to insert alert agent_id={} rule={} err={}",
                            agent.id, rule.event_type, e
                        )
                    );
                    continue;
                }
                crate::log_warn!(
                    "hardware_alerts",
                    &format!(
                        "agent_id={} alert={} message={}",
                        agent.id, rule.event_type, message
                    )
                );
            }
            (false, Some(open_event)) => {
                if let Err(e) = events::resolve(db, open_event.id).await {
                    crate::log_error!(
                        "hardware_alerts",
                        &format!(
                            "Failed to resolve alert agent_id={} rule={} err={}",
                            agent.id, rule.event_type, e
                        )
                    );
                    continue;
                }
                crate::log_info!(
                    "hardware_alerts",
                    &format!("agent_id={} alert={} resolved", agent.id, rule.event_type)
                );
            }
            _ => {}
        }
    }
}
