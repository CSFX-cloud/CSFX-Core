use entity::entities::logs;

const CEF_VERSION: u8 = 0;
const DEVICE_VENDOR: &str = "CSFX";
const DEVICE_PRODUCT: &str = "CSFX Core";

fn escape_header(value: &str) -> String {
    value.replace('\\', "\\\\").replace('|', "\\|")
}

fn escape_extension(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('=', "\\=")
        .replace('\r', "")
        .replace('\n', "\\n")
}

fn severity(level: &str) -> u8 {
    match level {
        "ERROR" => 8,
        "WARN" => 5,
        "INFO" => 3,
        _ => 1,
    }
}

fn signature_id(entry: &logs::Model) -> String {
    entry
        .action
        .clone()
        .unwrap_or_else(|| format!("{}.{}", entry.service, entry.classification))
}

fn extension_key(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    format!("csfx{cleaned}")
}

fn field_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn extension_pairs(entry: &logs::Model) -> Vec<(String, String)> {
    let mut pairs = vec![
        ("rt".to_string(), entry.last_seen_at.timestamp_millis().to_string()),
        ("externalId".to_string(), entry.id.to_string()),
        ("cat".to_string(), entry.classification.clone()),
        ("cs1Label".to_string(), "service".to_string()),
        ("cs1".to_string(), entry.service.clone()),
        ("cnt".to_string(), entry.occurrences.to_string()),
        ("msg".to_string(), entry.message.clone()),
    ];
    let optional = [
        ("act", entry.action.clone()),
        ("suser", entry.actor_user_id.map(|id| id.to_string())),
        ("cs2Label", entry.agent_id.map(|_| "agentId".to_string())),
        ("cs2", entry.agent_id.map(|id| id.to_string())),
        ("cs3Label", entry.workload_id.map(|_| "workloadId".to_string())),
        ("cs3", entry.workload_id.map(|id| id.to_string())),
        ("cs4Label", entry.target_type.as_ref().map(|_| "targetType".to_string())),
        ("cs4", entry.target_type.clone()),
        ("cs5Label", entry.target_id.as_ref().map(|_| "targetId".to_string())),
        ("cs5", entry.target_id.clone()),
    ];
    pairs.extend(
        optional
            .into_iter()
            .filter_map(|(key, value)| Some((key.to_string(), value?))),
    );
    if let Some(fields) = entry.fields.as_object() {
        pairs.extend(
            fields
                .iter()
                .map(|(key, value)| (extension_key(key), field_text(value))),
        );
    }
    pairs
}

pub fn format_cef(entry: &logs::Model) -> String {
    let extension = extension_pairs(entry)
        .into_iter()
        .map(|(key, value)| format!("{key}={}", escape_extension(&value)))
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        "CEF:{CEF_VERSION}|{DEVICE_VENDOR}|{DEVICE_PRODUCT}|{}|{}|{}|{}|{extension}",
        escape_header(env!("CARGO_PKG_VERSION")),
        escape_header(&signature_id(entry)),
        escape_header(&entry.message),
        severity(&entry.level),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample() -> logs::Model {
        let now = chrono::Utc::now().into();
        logs::Model {
            id: Uuid::nil(),
            service: "registry".to_string(),
            level: "ERROR".to_string(),
            classification: "system".to_string(),
            message: "bad | pipe=x".to_string(),
            agent_id: None,
            workload_id: None,
            organization_id: None,
            created_at: now,
            kind: "system".to_string(),
            actor_user_id: None,
            action: None,
            target_type: None,
            target_id: None,
            fields: serde_json::json!({ "unit": "sshd.service" }),
            occurrences: 2,
            last_seen_at: now,
        }
    }

    #[test]
    fn escapes_header_and_extension() {
        let line = format_cef(&sample());
        assert!(line.starts_with("CEF:0|CSFX|CSFX Core|"));
        assert!(line.contains("|registry.system|bad \\| pipe=x|8|"));
        assert!(line.contains("msg=bad | pipe\\=x"));
        assert!(line.contains("csfxunit=sshd.service"));
        assert!(line.contains("cnt=2"));
    }
}
