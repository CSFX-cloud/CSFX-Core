use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use crate::config::Config;
use crate::etcd::{self, Client};

const FRESH_SECS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Halted,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub version: String,
    pub order: Vec<String>,
    pub sizes: Vec<usize>,
    pub wave: usize,
    pub wave_started: u64,
    pub status: Status,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Wait,
    Advance,
    Halt,
}

pub fn cumulative_sizes(fleet: usize) -> Vec<usize> {
    let mut sizes: Vec<usize> = Vec::new();
    for target in [1, fleet.div_ceil(20), fleet.div_ceil(4)] {
        let target = target.min(fleet);
        if target > sizes.last().copied().unwrap_or(0) {
            sizes.push(target);
        }
    }
    sizes
}

pub fn decide(total: usize, failed: usize, elapsed: u64, bake: u64, max_percent: u64) -> Decision {
    if elapsed < bake {
        return Decision::Wait;
    }
    if failed as u64 * 100 > total as u64 * max_percent {
        return Decision::Halt;
    }
    Decision::Advance
}

pub fn plan(version: &str, order: Vec<String>, now: u64) -> State {
    let sizes = cumulative_sizes(order.len());
    let status = if sizes.is_empty() {
        Status::Complete
    } else {
        Status::Running
    };
    State {
        version: version.to_string(),
        order,
        sizes,
        wave: 0,
        wave_started: now,
        status,
    }
}

impl State {
    pub fn admitted(&self) -> Option<&[String]> {
        match self.status {
            Status::Complete => None,
            _ => Some(&self.order[..self.sizes[self.wave]]),
        }
    }

    pub fn wave_hosts(&self) -> &[String] {
        let start = match self.wave {
            0 => 0,
            wave => self.sizes[wave - 1],
        };
        &self.order[start..self.sizes[self.wave]]
    }
}

pub fn next(state: &State, failed: usize, now: u64, cfg: &Config) -> State {
    if state.status != Status::Running {
        return state.clone();
    }
    let elapsed = now.saturating_sub(state.wave_started);
    let total = state.wave_hosts().len();
    let mut updated = state.clone();
    match decide(
        total,
        failed,
        elapsed,
        cfg.wave_bake_secs,
        cfg.wave_max_failure_percent,
    ) {
        Decision::Wait => {}
        Decision::Halt => updated.status = Status::Halted,
        Decision::Advance if state.wave + 1 < state.sizes.len() => {
            updated.wave += 1;
            updated.wave_started = now;
        }
        Decision::Advance => updated.status = Status::Complete,
    }
    updated
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn parse_fleet_entry(key: &str, value: &str) -> Option<(String, u64, String)> {
    let id = key.strip_prefix(etcd::FLEET_PREFIX)?;
    let (seen, version) = value.split_once('|')?;
    Some((id.to_string(), seen.parse().ok()?, version.to_string()))
}

pub fn eligible_hosts(entries: &[(String, String)], now: u64) -> Vec<String> {
    let mut hosts: Vec<String> = entries
        .iter()
        .filter_map(|(key, value)| parse_fleet_entry(key, value))
        .filter(|(_, seen, version)| !version.is_empty() && now.saturating_sub(*seen) <= FRESH_SECS)
        .map(|(id, _, _)| id)
        .collect();
    hosts.sort();
    hosts
}

pub fn failed_hosts(entries: &[(String, String)], hosts: &[String], version: &str) -> usize {
    hosts
        .iter()
        .filter(|host| {
            let key = format!("{}{host}", etcd::FLEET_PREFIX);
            let reported = entries
                .iter()
                .find(|(entry_key, _)| *entry_key == key)
                .and_then(|(entry_key, value)| parse_fleet_entry(entry_key, value));
            reported.is_none_or(|(_, _, reported)| reported != version)
        })
        .count()
}

async fn load(etcd: &mut Client) -> Result<Option<State>> {
    match etcd.get(etcd::ROLLOUT_STATE_KEY).await? {
        Some(raw) => Ok(Some(serde_json::from_str(&raw)?)),
        None => Ok(None),
    }
}

async fn save(etcd: &mut Client, state: &State) -> Result<()> {
    match state.admitted() {
        Some(hosts) => {
            etcd.put(etcd::ROLLOUT_HOSTS_KEY, &serde_json::to_string(hosts)?)
                .await?
        }
        None => etcd.delete(etcd::ROLLOUT_HOSTS_KEY).await?,
    }
    etcd.put(etcd::ROLLOUT_STATE_KEY, &serde_json::to_string(state)?)
        .await
}

pub async fn begin(etcd: &mut Client, version: &str) -> Result<()> {
    if load(etcd)
        .await?
        .is_some_and(|state| state.version == version)
    {
        return Ok(());
    }
    let now = now_secs();
    let entries = etcd.entries_with_prefix(etcd::FLEET_PREFIX).await?;
    let state = plan(version, eligible_hosts(&entries, now), now);
    save(etcd, &state).await?;
    info!(
        version,
        hosts = state.order.len(),
        waves = state.sizes.len(),
        "worker rollout planned"
    );
    Ok(())
}

pub async fn advance(cfg: &Config, etcd: &mut Client) -> Result<()> {
    let Some(state) = load(etcd).await? else {
        return Ok(());
    };
    if state.status != Status::Running {
        return Ok(());
    }
    let entries = etcd.entries_with_prefix(etcd::FLEET_PREFIX).await?;
    let failed = failed_hosts(&entries, state.wave_hosts(), &state.version);
    let updated = next(&state, failed, now_secs(), cfg);
    if updated == state {
        return Ok(());
    }
    save(etcd, &updated).await?;
    report(&state, &updated, failed);
    Ok(())
}

fn report(before: &State, after: &State, failed: usize) {
    let version = before.version.as_str();
    match after.status {
        Status::Halted => warn!(
            version,
            wave = before.wave,
            failed,
            total = before.wave_hosts().len(),
            "worker rollout halted, failure rate above threshold"
        ),
        Status::Complete => info!(version, "worker rollout released to the whole fleet"),
        Status::Running => info!(version, wave = after.wave, "worker rollout advanced"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            etcd_endpoints: Vec::new(),
            poll_interval_secs: 1,
            state_dir: Default::default(),
            release_base_url: String::new(),
            release_public_key_file: Default::default(),
            peer_substituters: Vec::new(),
            upstream_substituters: Vec::new(),
            gateway_url: None,
            health_check_timeout_secs: 1,
            health_check_retry_interval_secs: 1,
            wave_bake_secs: 100,
            wave_max_failure_percent: 10,
        }
    }

    fn hosts(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("h{index:04}")).collect()
    }

    fn entry(host: &str, seen: u64, version: &str) -> (String, String) {
        (
            format!("{}{host}", etcd::FLEET_PREFIX),
            format!("{seen}|{version}"),
        )
    }

    #[test]
    fn wave_sizes_scale_with_the_fleet() {
        assert_eq!(cumulative_sizes(0), Vec::<usize>::new());
        assert_eq!(cumulative_sizes(1), vec![1]);
        assert_eq!(cumulative_sizes(3), vec![1]);
        assert_eq!(cumulative_sizes(10), vec![1, 3]);
        assert_eq!(cumulative_sizes(100), vec![1, 5, 25]);
        assert_eq!(cumulative_sizes(1000), vec![1, 50, 250]);
    }

    #[test]
    fn decision_waits_for_the_bake_time() {
        assert_eq!(decide(10, 10, 99, 100, 10), Decision::Wait);
    }

    #[test]
    fn decision_halts_above_the_threshold_only() {
        assert_eq!(decide(10, 1, 100, 100, 10), Decision::Advance);
        assert_eq!(decide(10, 2, 100, 100, 10), Decision::Halt);
        assert_eq!(decide(1, 1, 100, 100, 10), Decision::Halt);
        assert_eq!(decide(1, 0, 100, 100, 10), Decision::Advance);
    }

    #[test]
    fn an_empty_fleet_completes_immediately() {
        assert_eq!(plan("1", Vec::new(), 0).status, Status::Complete);
    }

    #[test]
    fn rollout_advances_through_waves_then_completes() {
        let mut state = plan("1", hosts(100), 0);
        assert_eq!(state.admitted().unwrap().len(), 1);
        state = next(&state, 0, 100, &config());
        assert_eq!((state.wave, state.admitted().unwrap().len()), (1, 5));
        assert_eq!(state.wave_hosts().len(), 4);
        state = next(&state, 0, 200, &config());
        assert_eq!(state.admitted().unwrap().len(), 25);
        state = next(&state, 0, 300, &config());
        assert_eq!(state.status, Status::Complete);
        assert!(state.admitted().is_none());
    }

    #[test]
    fn rollout_halts_and_stays_halted_on_failures() {
        let state = plan("1", hosts(100), 0);
        let halted = next(&state, 1, 100, &config());
        assert_eq!(halted.status, Status::Halted);
        assert_eq!(halted.admitted().unwrap().len(), 1);
        assert_eq!(next(&halted, 0, 1000, &config()), halted);
    }

    #[test]
    fn rollout_waits_before_the_bake_time_is_over() {
        let state = plan("1", hosts(100), 0);
        assert_eq!(next(&state, 1, 50, &config()), state);
    }

    #[test]
    fn only_fresh_hosts_with_a_known_release_are_eligible() {
        let entries = vec![
            entry("b", 1000, "1"),
            entry("a", 1000, "1"),
            entry("stale", 1, "1"),
            entry("unknown", 1000, ""),
            ("/other/x".to_string(), "1000|1".to_string()),
        ];
        assert_eq!(eligible_hosts(&entries, 1100), vec!["a", "b"]);
    }

    #[test]
    fn hosts_not_on_the_target_release_count_as_failed() {
        let entries = vec![entry("a", 1, "2"), entry("b", 1, "1")];
        let wave = vec!["a".to_string(), "b".to_string(), "gone".to_string()];
        assert_eq!(failed_hosts(&entries, &wave, "2"), 2);
    }
}
