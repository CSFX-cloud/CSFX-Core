use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStdout, Command};
use tokio::time::{interval, sleep, MissedTickBehavior};
use tracing::{error, warn};

use crate::client::{ApiClient, JournalEntry};

const CURSOR_FILE: &str = "/var/lib/csfx/journal.cursor";
const MAX_PRIORITY: &str = "warning";
const BATCH_SIZE: usize = 100;
const FLUSH_INTERVAL: Duration = Duration::from_secs(2);
const RESTART_DELAY: Duration = Duration::from_secs(5);
const ERROR_PRIORITY_MAX: u8 = 3;

#[derive(Deserialize)]
struct RawJournalLine {
    #[serde(rename = "MESSAGE")]
    message: Option<serde_json::Value>,
    #[serde(rename = "PRIORITY")]
    priority: Option<String>,
    #[serde(rename = "_SYSTEMD_UNIT")]
    unit: Option<String>,
    #[serde(rename = "SYSLOG_IDENTIFIER")]
    identifier: Option<String>,
    #[serde(rename = "__REALTIME_TIMESTAMP")]
    timestamp_micros: Option<String>,
}

fn message_text(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text),
        serde_json::Value::Array(bytes) => {
            let raw: Vec<u8> = bytes.iter().filter_map(|b| b.as_u64()).map(|b| b as u8).collect();
            Some(String::from_utf8_lossy(&raw).into_owned())
        }
        _ => None,
    }
}

fn parse_line(line: &str) -> Option<JournalEntry> {
    let raw: RawJournalLine = serde_json::from_str(line).ok()?;
    let priority: u8 = raw.priority.as_deref()?.parse().ok()?;
    let message = message_text(raw.message?)?;
    Some(JournalEntry {
        level: if priority <= ERROR_PRIORITY_MAX { "ERROR" } else { "WARN" }.to_string(),
        unit: raw.unit.or(raw.identifier).unwrap_or_default(),
        message,
        timestamp_micros: raw.timestamp_micros.and_then(|t| t.parse().ok()),
    })
}

fn spawn_journalctl() -> Result<(tokio::process::Child, ChildStdout)> {
    let mut child = Command::new("journalctl")
        .args(["-o", "json", "-f", "-n", "0", "-p", MAX_PRIORITY])
        .arg(format!("--cursor-file={CURSOR_FILE}"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("Failed to spawn journalctl")?;
    let stdout = child.stdout.take().context("journalctl stdout unavailable")?;
    Ok((child, stdout))
}

async fn ship(client: &ApiClient, api_key: &str, batch: &mut Vec<JournalEntry>, failing: &mut bool) {
    if batch.is_empty() {
        return;
    }
    let entries = std::mem::take(batch);
    match client.push_logs(api_key, entries).await {
        Ok(()) => *failing = false,
        Err(e) if !*failing => {
            *failing = true;
            warn!(error = %e, "log shipping failed");
        }
        Err(_) => {}
    }
}

async fn stream_once(client: &ApiClient, api_key: &str) -> Result<()> {
    let (_child, stdout) = spawn_journalctl()?;
    let mut lines = BufReader::new(stdout).lines();
    let mut ticker = interval(FLUSH_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut failing = false;

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line.context("Failed to read journal output")? else {
                    ship(client, api_key, &mut batch, &mut failing).await;
                    return Ok(());
                };
                batch.extend(parse_line(&line));
                if batch.len() >= BATCH_SIZE {
                    ship(client, api_key, &mut batch, &mut failing).await;
                }
            }
            _ = ticker.tick() => ship(client, api_key, &mut batch, &mut failing).await,
        }
    }
}

pub fn spawn(client: Arc<ApiClient>, api_key: String) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = stream_once(&client, &api_key).await {
                error!(error = %e, "journal streaming stopped");
            }
            sleep(RESTART_DELAY).await;
        }
    });
}
