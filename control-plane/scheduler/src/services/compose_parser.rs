use std::collections::HashMap;

use crate::models::compose::{ComposeServiceSpec, ComposeVolumeMount};
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
    #[error(
        "service '{0}' uses a bind mount or anonymous volume, only named volumes are supported"
    )]
    UnsupportedVolumeSource(String),
    #[error("service '{service}' mounts volume '{volume}' that is not declared under volumes")]
    UndeclaredVolume { service: String, volume: String },
    #[error("service '{0}' has a volume mount path that is not absolute")]
    InvalidMountPath(String),
}

#[derive(Debug, serde::Deserialize)]
struct RawCompose {
    services: HashMap<String, RawService>,
    #[serde(default)]
    volumes: Option<HashMap<String, Option<RawVolumeDefinition>>>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct RawVolumeDefinition {
    external: Option<RawExternal>,
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum RawExternal {
    Flag(bool),
    Named { name: String },
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum RawVolumeMount {
    Short(String),
    Long {
        #[serde(rename = "type")]
        kind: Option<String>,
        source: Option<String>,
        target: String,
    },
}

#[derive(Debug, serde::Deserialize)]
struct RawService {
    image: Option<String>,
    build: Option<serde_yaml::Value>,
    ports: Option<Vec<String>>,
    volumes: Option<Vec<RawVolumeMount>>,
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

    let definitions = raw.volumes.unwrap_or_default();
    raw.services
        .into_iter()
        .map(|(service_name, service)| build_service_spec(service_name, service, &definitions))
        .collect()
}

fn build_service_spec(
    service_name: String,
    service: RawService,
    definitions: &HashMap<String, Option<RawVolumeDefinition>>,
) -> Result<ComposeServiceSpec, ComposeParseError> {
    if service.build.is_some() {
        return Err(ComposeParseError::UnsupportedBuildDirective(service_name));
    }
    let image = service
        .image
        .ok_or_else(|| ComposeParseError::MissingImage(service_name.clone()))?;
    let resources = resolve_resources(service.deploy, &service_name)?;
    let volumes = parse_volume_mounts(service.volumes, definitions, &service_name)?;

    Ok(ComposeServiceSpec {
        service_name,
        image,
        env_vars: normalize_environment(service.environment),
        ports: service.ports.map(|raw| parse_compose_ports(&raw)),
        volumes,
        cpu_millicores: resources.cpu_millicores,
        memory_bytes: resources.memory_bytes,
        disk_bytes: DEFAULT_DISK_BYTES,
        cpu_limit_millicores: resources.cpu_limit_millicores,
        memory_limit_bytes: resources.memory_limit_bytes,
    })
}

fn parse_volume_mounts(
    raw: Option<Vec<RawVolumeMount>>,
    definitions: &HashMap<String, Option<RawVolumeDefinition>>,
    service_name: &str,
) -> Result<Vec<ComposeVolumeMount>, ComposeParseError> {
    raw.unwrap_or_default()
        .into_iter()
        .map(|mount| {
            let (source, target) = split_volume_mount(mount, service_name)?;
            if !target.starts_with('/') {
                return Err(ComposeParseError::InvalidMountPath(
                    service_name.to_string(),
                ));
            }
            let definition =
                definitions
                    .get(&source)
                    .ok_or_else(|| ComposeParseError::UndeclaredVolume {
                        service: service_name.to_string(),
                        volume: source.clone(),
                    })?;
            Ok(ComposeVolumeMount {
                external_name: external_name_of(&source, definition.as_ref()),
                source,
                mount_path: target,
            })
        })
        .collect()
}

fn split_volume_mount(
    mount: RawVolumeMount,
    service_name: &str,
) -> Result<(String, String), ComposeParseError> {
    let unsupported = || ComposeParseError::UnsupportedVolumeSource(service_name.to_string());
    match mount {
        RawVolumeMount::Short(entry) => {
            let mut parts = entry.split(':');
            let source = parts.next().unwrap_or_default();
            let target = parts.next().ok_or_else(unsupported)?;
            let is_path = source.starts_with(['.', '/', '~']);
            if source.is_empty() || is_path {
                return Err(unsupported());
            }
            Ok((source.to_string(), target.to_string()))
        }
        RawVolumeMount::Long {
            kind,
            source,
            target,
        } => match (kind.as_deref(), source) {
            (Some("volume"), Some(source)) => Ok((source, target)),
            _ => Err(unsupported()),
        },
    }
}

fn external_name_of(source: &str, definition: Option<&RawVolumeDefinition>) -> Option<String> {
    let definition = definition?;
    match &definition.external {
        Some(RawExternal::Flag(true)) => Some(
            definition
                .name
                .clone()
                .unwrap_or_else(|| source.to_string()),
        ),
        Some(RawExternal::Named { name }) => Some(name.clone()),
        _ => None,
    }
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

    #[test]
    fn named_volumes_become_mounts() {
        let spec = parse_single(
            "services:\n  db:\n    image: postgres\n    volumes:\n      - data:/var/lib/data\n      - type: volume\n        source: logs\n        target: /logs\nvolumes:\n  data:\n  logs:\n    external: true\n",
        );
        assert_eq!(spec.volumes.len(), 2);
        assert_eq!(spec.volumes[0].source, "data");
        assert_eq!(spec.volumes[0].external_name, None);
        assert_eq!(spec.volumes[0].mount_path, "/var/lib/data");
        assert_eq!(spec.volumes[1].external_name.as_deref(), Some("logs"));
    }

    #[test]
    fn rejects_bind_mounts_and_undeclared_volumes() {
        let bind = parse_compose(
            "services:\n  web:\n    image: nginx\n    volumes:\n      - ./html:/usr/share/nginx/html\n",
        );
        assert!(matches!(
            bind,
            Err(ComposeParseError::UnsupportedVolumeSource(_))
        ));
        let undeclared = parse_compose(
            "services:\n  web:\n    image: nginx\n    volumes:\n      - data:/data\n",
        );
        assert!(matches!(
            undeclared,
            Err(ComposeParseError::UndeclaredVolume { .. })
        ));
    }
}
