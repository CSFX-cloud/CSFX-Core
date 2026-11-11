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
const IGNORED_UNIT_PREFIXES: [&str; 1] = ["serial-getty@"];

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
            let raw: Vec<u8> = bytes
                .iter()
                .filter_map(|b| b.as_u64())
                .map(|b| b as u8)
                .collect();
            Some(String::from_utf8_lossy(&raw).into_owned())
        }
        _ => None,
    }
}

fn is_ignored(unit: &str, message: &str) -> bool {
    IGNORED_UNIT_PREFIXES
        .iter()
        .any(|prefix| unit.starts_with(prefix) || message.starts_with(prefix))
}

fn parse_line(line: &str) -> Option<JournalEntry> {
    let raw: RawJournalLine = serde_json::from_str(line).ok()?;
    let priority: u8 = raw.priority.as_deref()?.parse().ok()?;
    let message = message_text(raw.message?)?;
    let unit = raw.unit.or(raw.identifier).unwrap_or_default();
    if is_ignored(&unit, &message) {
        return None;
    }
    Some(JournalEntry {
        level: if priority <= ERROR_PRIORITY_MAX {
            "ERROR"
        } else {
            "WARN"
        }
        .to_string(),
        unit,
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
    let stdout = child
        .stdout
        .take()
        .context("journalctl stdout unavailable")?;
    Ok((child, stdout))
}

async fn ship(
    client: &ApiClient,
    api_key: &str,
    batch: &mut Vec<JournalEntry>,
    failing: &mut bool,
) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn error_priorities_map_to_error_level() {
        let line = json!({
            "MESSAGE": "boom",
            "PRIORITY": "3",
            "_SYSTEMD_UNIT": "app.service",
            "__REALTIME_TIMESTAMP": "1700000000000000"
        })
        .to_string();
        let entry = parse_line(&line).unwrap();
        assert_eq!(entry.level, "ERROR");
        assert_eq!(entry.unit, "app.service");
        assert_eq!(entry.message, "boom");
        assert_eq!(entry.timestamp_micros, Some(1_700_000_000_000_000));
    }

    #[test]
    fn warning_priorities_fall_back_to_the_syslog_identifier() {
        let line = json!({
            "MESSAGE": "careful",
            "PRIORITY": "4",
            "SYSLOG_IDENTIFIER": "sshd"
        })
        .to_string();
        let entry = parse_line(&line).unwrap();
        assert_eq!(entry.level, "WARN");
        assert_eq!(entry.unit, "sshd");
        assert!(entry.timestamp_micros.is_none());
    }

    #[test]
    fn byte_array_messages_are_decoded() {
        let line = json!({"MESSAGE": [104, 105], "PRIORITY": "3"}).to_string();
        assert_eq!(parse_line(&line).unwrap().message, "hi");
        assert!(message_text(json!(5)).is_none());
    }

    #[test]
    fn invalid_and_ignored_lines_are_dropped() {
        assert!(parse_line("not json").is_none());
        assert!(parse_line(&json!({"MESSAGE": "x"}).to_string()).is_none());
        assert!(parse_line(&json!({"PRIORITY": "3"}).to_string()).is_none());
        assert!(parse_line(&json!({"MESSAGE": "x", "PRIORITY": "high"}).to_string()).is_none());
        let getty = json!({
            "MESSAGE": "x",
            "PRIORITY": "3",
            "_SYSTEMD_UNIT": "serial-getty@ttyS0.service"
        })
        .to_string();
        assert!(parse_line(&getty).is_none());
        assert!(is_ignored("other", "serial-getty@ttyS0 failed"));
        assert!(!is_ignored("other", "fine"));
    }

    #[tokio::test]
    async fn shipping_pushes_batches_and_tracks_failures() {
        use axum::{http::StatusCode, routing::post, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};

        crate::client::install_test_crypto_provider();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let app = Router::new().route(
            "/api/agents/self/logs",
            post(move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = ApiClient::new(url, "pub".to_string(), None, None, None).unwrap();

        let mut batch = Vec::new();
        let mut failing = false;
        ship(&client, "key", &mut batch, &mut failing).await;
        assert_eq!(hits.load(Ordering::SeqCst), 0);

        for _ in 0..2 {
            batch.extend(parse_line(
                &json!({"MESSAGE": "m", "PRIORITY": "3"}).to_string(),
            ));
            ship(&client, "key", &mut batch, &mut failing).await;
            assert!(batch.is_empty());
            assert!(failing);
        }
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}
