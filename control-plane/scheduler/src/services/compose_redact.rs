use serde_yaml::Value;
use shared::env_secrets::{parse_vault_ref, MASK};

pub fn redact(source: &str) -> Option<String> {
    let mut document: Value = serde_yaml::from_str(source).ok()?;
    if let Some(root) = document.as_mapping_mut() {
        root.retain(|key, _| !key.as_str().is_some_and(|name| name.starts_with("x-")));
    }
    if let Some(services) = document.get_mut("services").and_then(Value::as_mapping_mut) {
        services.values_mut().for_each(mask_service);
    }
    serde_yaml::to_string(&document).ok()
}

fn mask_service(service: &mut Value) {
    if let Some(environment) = service.get_mut("environment") {
        mask_environment(environment);
    }
    match service.get_mut("<<") {
        Some(Value::Sequence(parents)) => parents.iter_mut().for_each(mask_service),
        Some(parent) => mask_service(parent),
        None => {}
    }
}

fn mask_environment(environment: &mut Value) {
    match environment {
        Value::Mapping(entries) => entries
            .values_mut()
            .filter(|value| !value.is_null() && !is_reference(value))
            .for_each(|value| *value = Value::String(MASK.to_string())),
        Value::Sequence(items) => items.iter_mut().for_each(mask_assignment),
        _ => {}
    }
}

fn is_reference(value: &Value) -> bool {
    value.as_str().and_then(parse_vault_ref).is_some()
}

fn mask_assignment(item: &mut Value) {
    let Some(text) = item.as_str() else {
        return;
    };
    let Some((name, value)) = text.split_once('=') else {
        return;
    };
    if parse_vault_ref(value).is_some() {
        return;
    }
    *item = Value::String(format!("{name}={MASK}"));
}

pub fn restore(submitted: &str, previous: &str) -> Option<String> {
    let mut document: Value = serde_yaml::from_str(submitted).ok()?;
    let earlier: Value = serde_yaml::from_str(previous).ok()?;
    let services = document.get_mut("services")?.as_mapping_mut()?;
    for (name, service) in services.iter_mut() {
        let before = earlier
            .get("services")
            .and_then(|all| all.get(name))
            .and_then(|service| service.get("environment"));
        if let (Some(environment), Some(before)) = (service.get_mut("environment"), before) {
            restore_environment(environment, before);
        }
    }
    serde_yaml::to_string(&document).ok()
}

fn restore_environment(environment: &mut Value, before: &Value) {
    match environment {
        Value::Mapping(entries) => entries
            .iter_mut()
            .filter(|(_, value)| value.as_str() == Some(MASK))
            .for_each(|(name, value)| {
                if let Some(old) = name.as_str().and_then(|name| earlier_value(before, name)) {
                    *value = old;
                }
            }),
        Value::Sequence(items) => items.iter_mut().for_each(|item| {
            let Some((name, MASK)) = item.as_str().and_then(|text| text.split_once('=')) else {
                return;
            };
            if let Some(old) = earlier_value(before, name) {
                *item = Value::String(format!("{name}={}", scalar_text(&old)));
            }
        }),
        _ => {}
    }
}

fn earlier_value(before: &Value, name: &str) -> Option<Value> {
    match before {
        Value::Mapping(entries) => entries.get(name).cloned(),
        Value::Sequence(items) => items
            .iter()
            .filter_map(Value::as_str)
            .find_map(|text| text.strip_prefix(name)?.strip_prefix('='))
            .map(|rest| Value::String(rest.to_string())),
        _ => None,
    }
}

fn scalar_text(value: &Value) -> String {
    value.as_str().map_or_else(
        || {
            serde_yaml::to_string(value)
                .unwrap_or_default()
                .trim()
                .to_string()
        },
        str::to_string,
    )
}

#[cfg(test)]
mod tests {
    use super::{redact, restore};

    #[test]
    fn mapping_and_list_values_are_masked() {
        let source = "services:\n  web:\n    image: nginx\n    environment:\n      TOKEN: hunter2\n      EMPTY:\n  db:\n    environment:\n      - PASSWORD=hunter2\n      - PASSTHROUGH\n";
        let redacted = redact(source).unwrap();
        assert!(!redacted.contains("hunter2"));
        assert!(redacted.contains("PASSWORD=********"));
        assert!(redacted.contains("PASSTHROUGH"));
        assert!(redacted.contains("image: nginx"));
    }

    #[test]
    fn extension_blocks_and_merged_environments_are_hidden() {
        let source = "x-env: &env\n  environment:\n    TOKEN: hunter2\nservices:\n  web:\n    <<: *env\n    image: nginx\n";
        let redacted = redact(source).unwrap();
        assert!(!redacted.contains("hunter2"));
        assert!(redacted.contains("image: nginx"));
    }

    #[test]
    fn masked_values_are_restored_from_the_previous_source() {
        let previous = "services:\n  web:\n    environment:\n      TOKEN: hunter2\n  db:\n    environment:\n      - PASSWORD=s3cret\n";
        let submitted = "services:\n  web:\n    environment:\n      TOKEN: '********'\n      NEW: fresh\n  db:\n    environment:\n      - PASSWORD=********\n";
        let restored = restore(submitted, previous).unwrap();
        assert!(restored.contains("TOKEN: hunter2"));
        assert!(restored.contains("PASSWORD=s3cret"));
        assert!(restored.contains("NEW: fresh"));
        assert!(!restored.contains("********"));
    }

    #[test]
    fn vault_references_stay_visible() {
        let id = uuid::Uuid::new_v4();
        let source = format!(
            "services:\n  web:\n    environment:\n      A: vault://{id}/KEY\n      B: plain\n  db:\n    environment:\n      - C=vault://{id}/KEY\n"
        );
        let redacted = redact(&source).unwrap();
        assert!(redacted.contains(&format!("A: vault://{id}/KEY")));
        assert!(redacted.contains(&format!("C=vault://{id}/KEY")));
        assert!(!redacted.contains("plain"));
    }

    #[test]
    fn invalid_yaml_is_not_returned() {
        assert!(redact("services: [").is_none());
    }
}
