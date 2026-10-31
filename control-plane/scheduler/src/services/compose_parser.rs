use std::collections::HashMap;

use crate::models::compose::ComposeServiceSpec;
use crate::models::workload::PortMapping;

const DEFAULT_CPU_MILLICORES: i32 = 500;
const DEFAULT_MEMORY_BYTES: i64 = 512 * 1024 * 1024;
const DEFAULT_DISK_BYTES: i64 = 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ComposeParseError {
    #[error("invalid compose yaml: {0}")]
    InvalidYaml(String),
    #[error("compose file defines no services")]
    NoServicesDefined,
    #[error("service '{0}' uses build, only image is supported")]
    UnsupportedBuildDirective(String),
    #[error("service '{0}' has no image")]
    MissingImage(String),
    #[error("service '{0}' has invalid deploy resources")]
    InvalidResources(String),
}

#[derive(Debug, serde::Deserialize)]
struct RawCompose {
    services: HashMap<String, RawService>,
}

#[derive(Debug, serde::Deserialize)]
struct RawService {
    image: Option<String>,
    build: Option<serde_yaml::Value>,
    ports: Option<Vec<String>>,
    environment: Option<RawEnvironment>,
    deploy: Option<RawDeploy>,
}

#[derive(Debug, serde::Deserialize)]
struct RawDeploy {
    resources: Option<RawResources>,
}

#[derive(Debug, serde::Deserialize)]
struct RawResources {
    limits: Option<RawAmounts>,
    reservations: Option<RawAmounts>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct RawAmounts {
    cpus: Option<serde_yaml::Value>,
    memory: Option<serde_yaml::Value>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum RawEnvironment {
    List(Vec<String>),
    Map(HashMap<String, String>),
}

pub fn parse_compose(yaml: &str) -> Result<Vec<ComposeServiceSpec>, ComposeParseError> {
    let raw: RawCompose =
        serde_yaml::from_str(yaml).map_err(|e| ComposeParseError::InvalidYaml(e.to_string()))?;

    if raw.services.is_empty() {
        return Err(ComposeParseError::NoServicesDefined);
    }

    raw.services
        .into_iter()
        .map(|(service_name, service)| build_service_spec(service_name, service))
        .collect()
}

fn build_service_spec(
    service_name: String,
    service: RawService,
) -> Result<ComposeServiceSpec, ComposeParseError> {
    if service.build.is_some() {
        return Err(ComposeParseError::UnsupportedBuildDirective(service_name));
    }
    let image = service
        .image
        .ok_or_else(|| ComposeParseError::MissingImage(service_name.clone()))?;
    let resources = resolve_resources(service.deploy, &service_name)?;

    Ok(ComposeServiceSpec {
        service_name,
        image,
        env_vars: normalize_environment(service.environment),
        ports: service.ports.map(|raw| parse_compose_ports(&raw)),
        cpu_millicores: resources.cpu_millicores,
        memory_bytes: resources.memory_bytes,
        disk_bytes: DEFAULT_DISK_BYTES,
        cpu_limit_millicores: resources.cpu_limit_millicores,
        memory_limit_bytes: resources.memory_limit_bytes,
    })
}

struct ResolvedResources {
    cpu_millicores: i32,
    memory_bytes: i64,
    cpu_limit_millicores: Option<i32>,
    memory_limit_bytes: Option<i64>,
}

fn resolve_resources(
    deploy: Option<RawDeploy>,
    service_name: &str,
) -> Result<ResolvedResources, ComposeParseError> {
    let invalid = || ComposeParseError::InvalidResources(service_name.to_string());
    let resources = deploy.and_then(|d| d.resources);
    let (limits, reservations) = match resources {
        Some(r) => (
            r.limits.unwrap_or_default(),
            r.reservations.unwrap_or_default(),
        ),
        None => Default::default(),
    };

    let cpu_limit = parse_cpus(limits.cpus.as_ref()).map_err(|_| invalid())?;
    let memory_limit = parse_memory(limits.memory.as_ref()).map_err(|_| invalid())?;
    let cpu_request = parse_cpus(reservations.cpus.as_ref()).map_err(|_| invalid())?;
    let memory_request = parse_memory(reservations.memory.as_ref()).map_err(|_| invalid())?;

    Ok(ResolvedResources {
        cpu_millicores: cpu_request
            .or(cpu_limit)
            .unwrap_or(DEFAULT_CPU_MILLICORES)
            .min(cpu_limit.unwrap_or(i32::MAX)),
        memory_bytes: memory_request
            .or(memory_limit)
            .unwrap_or(DEFAULT_MEMORY_BYTES)
            .min(memory_limit.unwrap_or(i64::MAX)),
        cpu_limit_millicores: cpu_limit,
        memory_limit_bytes: memory_limit,
    })
}

fn parse_cpus(value: Option<&serde_yaml::Value>) -> Result<Option<i32>, ()> {
    let cpus = match value {
        None => return Ok(None),
        Some(serde_yaml::Value::Number(number)) => number.as_f64().ok_or(())?,
        Some(serde_yaml::Value::String(text)) => text.trim().parse::<f64>().map_err(|_| ())?,
        Some(_) => return Err(()),
    };
    if !cpus.is_finite() || cpus <= 0.0 {
        return Err(());
    }
    Ok(Some((cpus * 1000.0).round() as i32))
}

fn parse_memory(value: Option<&serde_yaml::Value>) -> Result<Option<i64>, ()> {
    let text = match value {
        None => return Ok(None),
        Some(serde_yaml::Value::Number(number)) => return number.as_i64().map(Some).ok_or(()),
        Some(serde_yaml::Value::String(text)) => text.trim().to_ascii_lowercase(),
        Some(_) => return Err(()),
    };
    let digits_end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let amount: i64 = text[..digits_end].parse().map_err(|_| ())?;
    let multiplier: i64 = match text[digits_end..].trim_end_matches('b') {
        "" => 1,
        "k" => 1024,
        "m" => 1024 * 1024,
        "g" => 1024 * 1024 * 1024,
        _ => return Err(()),
    };
    amount.checked_mul(multiplier).map(Some).ok_or(())
}

fn normalize_environment(raw: Option<RawEnvironment>) -> Option<HashMap<String, String>> {
    match raw? {
        RawEnvironment::Map(map) => Some(map),
        RawEnvironment::List(list) => Some(
            list.into_iter()
                .filter_map(|entry| {
                    let (key, value) = entry.split_once('=')?;
                    Some((key.to_string(), value.to_string()))
                })
                .collect(),
        ),
    }
}

fn parse_compose_ports(raw: &[String]) -> Vec<PortMapping> {
    raw.iter()
        .filter_map(|entry| parse_compose_port(entry))
        .collect()
}

fn parse_compose_port(entry: &str) -> Option<PortMapping> {
    let (rg_port, container_port) = match entry.split_once(':') {
        Some((host, container)) => (host.parse().ok(), container),
        None => (None, entry),
    };

    Some(PortMapping {
        container_port: container_port.parse().ok()?,
        protocol: None,
        rg_port,
        node_port: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_single(yaml: &str) -> ComposeServiceSpec {
        parse_compose(yaml).unwrap().remove(0)
    }

    #[test]
    fn reservations_become_request_and_limits_become_ceiling() {
        let spec = parse_single(
            "services:\n  web:\n    image: nginx\n    deploy:\n      resources:\n        limits:\n          cpus: '2'\n          memory: 1G\n        reservations:\n          cpus: '0.5'\n          memory: 256M\n",
        );
        assert_eq!(spec.cpu_millicores, 500);
        assert_eq!(spec.cpu_limit_millicores, Some(2000));
        assert_eq!(spec.memory_bytes, 256 * 1024 * 1024);
        assert_eq!(spec.memory_limit_bytes, Some(1024 * 1024 * 1024));
    }

    #[test]
    fn limits_alone_make_a_fixed_workload() {
        let spec = parse_single(
            "services:\n  web:\n    image: nginx\n    deploy:\n      resources:\n        limits:\n          cpus: '1'\n          memory: 512M\n",
        );
        assert_eq!(spec.cpu_millicores, 1000);
        assert_eq!(spec.memory_bytes, 512 * 1024 * 1024);
    }

    #[test]
    fn missing_deploy_uses_defaults_without_limits() {
        let spec = parse_single("services:\n  web:\n    image: nginx\n");
        assert_eq!(spec.cpu_millicores, DEFAULT_CPU_MILLICORES);
        assert_eq!(spec.cpu_limit_millicores, None);
    }

    #[test]
    fn rejects_unparseable_memory() {
        let result = parse_compose(
            "services:\n  web:\n    image: nginx\n    deploy:\n      resources:\n        limits:\n          memory: lots\n",
        );
        assert!(matches!(
            result,
            Err(ComposeParseError::InvalidResources(_))
        ));
    }
}
