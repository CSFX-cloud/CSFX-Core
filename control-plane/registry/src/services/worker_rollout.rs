use uuid::Uuid;

pub const HOSTS_KEY: &str = "/csfx/updater/rollout/hosts";
pub const FLEET_PREFIX: &str = "/csfx/updater/fleet/";

pub fn fleet_key(agent_id: Uuid) -> String {
    format!("{FLEET_PREFIX}{agent_id}")
}

pub fn fleet_value(seen_unix_secs: u64, release_version: &str) -> String {
    format!("{seen_unix_secs}|{release_version}")
}

pub fn admits(hosts: Option<&str>, agent_id: Uuid) -> bool {
    let Some(raw) = hosts else {
        return true;
    };
    match serde_json::from_str::<Vec<Uuid>>(raw) {
        Ok(hosts) => hosts.contains(&agent_id),
        Err(error) => {
            tracing::warn!(error = %error, "unreadable rollout host list, withholding release");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everyone_is_admitted_without_a_host_list() {
        assert!(admits(None, Uuid::new_v4()));
    }

    #[test]
    fn only_listed_hosts_are_admitted() {
        let listed = Uuid::new_v4();
        let raw = format!("[\"{listed}\"]");
        assert!(admits(Some(&raw), listed));
        assert!(!admits(Some(&raw), Uuid::new_v4()));
    }

    #[test]
    fn an_unreadable_host_list_admits_nobody() {
        assert!(!admits(Some("not json"), Uuid::new_v4()));
    }

    #[test]
    fn fleet_entries_carry_time_and_version() {
        assert_eq!(fleet_value(42, "0.3.0"), "42|0.3.0");
        assert_eq!(fleet_value(42, ""), "42|");
    }
}
