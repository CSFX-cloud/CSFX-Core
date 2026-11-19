use crate::config::Config;
use crate::error::ServiceError;
use crate::token::deployment_token;
use serde::Deserialize;
use serde_json::{json, Value};
use shared::{detect::Mode, images::SITE_IMAGE_HOST, validate};
use std::collections::BTreeMap;
use uuid::Uuid;

const GIB: i64 = 1024 * 1024 * 1024;
const RUNTIME_CPU_MILLICORES: i32 = 500;
const RUNTIME_MEMORY_BYTES: i64 = 512 * 1024 * 1024;
const RUNTIME_DISK_BYTES: i64 = 1024 * 1024 * 1024;
const SHORT_ID_LENGTH: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildSize {
    #[default]
    Small,
    Medium,
    Large,
}

impl BuildSize {
    fn resources(self) -> (i32, i64, i64) {
        match self {
            BuildSize::Small => (2000, 2 * GIB, 8 * GIB),
            BuildSize::Medium => (4000, 8 * GIB, 16 * GIB),
            BuildSize::Large => (8000, 16 * GIB, 32 * GIB),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildJob {
    pub deployment_id: Uuid,
    pub site_id: Uuid,
    pub resource_group_id: Uuid,
    pub site_name: String,
    pub mode: Mode,
    pub repo_url: String,
    pub branch: String,
    #[serde(default)]
    pub root_dir: String,
    pub framework: Option<String>,
    pub install_command: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub start_command: Option<String>,
    pub container_port: Option<u16>,
    #[serde(default)]
    pub build_size: BuildSize,
    pub git_token: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

pub fn short_id(id: Uuid) -> String {
    id.simple().to_string()[..SHORT_ID_LENGTH].to_string()
}

fn deployment_reference(registry: &str, site_id: Uuid, deployment_id: Uuid) -> String {
    format!(
        "{}/{}:{}",
        registry.trim_end_matches('/'),
        site_id,
        short_id(deployment_id)
    )
}

pub fn image_reference(registry: &str, job: &BuildJob) -> String {
    deployment_reference(registry, job.site_id, job.deployment_id)
}

pub fn stored_reference(site_id: Uuid, deployment_id: Uuid) -> String {
    deployment_reference(SITE_IMAGE_HOST, site_id, deployment_id)
}

impl BuildJob {
    pub fn validate(&self, config: &Config) -> Result<(), ServiceError> {
        let invalid = |message: &str| ServiceError::Invalid(message.to_string());
        validate::repo_url(&self.repo_url).map_err(invalid)?;
        validate::branch(&self.branch).map_err(invalid)?;
        validate::root_dir(&self.root_dir).map_err(invalid)?;
        for command in [
            &self.install_command,
            &self.build_command,
            &self.start_command,
        ]
        .into_iter()
        .flatten()
        {
            validate::command(command).map_err(invalid)?;
        }
        for name in self.env.keys() {
            validate::env_name(name).map_err(invalid)?;
        }
        if self.mode == Mode::Container && config.registry.is_none() {
            return Err(invalid("container mode requires CSFX_SITE_REGISTRY"));
        }
        if self.mode == Mode::Container && self.container_port.is_none() {
            return Err(invalid("container mode requires a container port"));
        }
        Ok(())
    }
}

fn optional_env(env: &mut BTreeMap<String, String>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        env.insert(name.to_string(), value.to_string());
    }
}

fn registry_settings(env: &mut BTreeMap<String, String>, config: &Config) {
    if config.registry_insecure {
        env.insert("CSFX_REGISTRY_INSECURE".to_string(), "1".to_string());
    }
    optional_env(
        env,
        "CSFX_REGISTRY_MIRROR",
        config.registry_mirror.as_deref(),
    );
}

fn builder_environment(config: &Config, job: &BuildJob) -> BTreeMap<String, String> {
    let mode = if job.mode == Mode::Static {
        "static"
    } else {
        "container"
    };
    let mut env = BTreeMap::from([
        (
            "CSFX_DEPLOYMENT_ID".to_string(),
            job.deployment_id.to_string(),
        ),
        ("CSFX_CALLBACK_URL".to_string(), config.callback_url.clone()),
        (
            "CSFX_CALLBACK_TOKEN".to_string(),
            deployment_token(&config.secret, job.deployment_id),
        ),
        ("CSFX_MODE".to_string(), mode.to_string()),
        ("CSFX_REPO_URL".to_string(), job.repo_url.clone()),
        ("CSFX_BRANCH".to_string(), job.branch.clone()),
        ("CSFX_ROOT_DIR".to_string(), job.root_dir.clone()),
        (
            "CSFX_BUILD_ENV".to_string(),
            serde_json::to_string(&job.env).unwrap_or_else(|_| "{}".to_string()),
        ),
    ]);
    optional_env(&mut env, "CSFX_FRAMEWORK", job.framework.as_deref());
    optional_env(
        &mut env,
        "CSFX_INSTALL_COMMAND",
        job.install_command.as_deref(),
    );
    optional_env(&mut env, "CSFX_BUILD_COMMAND", job.build_command.as_deref());
    optional_env(&mut env, "CSFX_OUTPUT_DIR", job.output_dir.as_deref());
    optional_env(&mut env, "CSFX_START_COMMAND", job.start_command.as_deref());
    optional_env(&mut env, "CSFX_GIT_TOKEN", job.git_token.as_deref());
    if let (Some(registry), Mode::Container) = (&config.registry, job.mode) {
        env.insert("CSFX_IMAGE_REF".to_string(), image_reference(registry, job));
        registry_settings(&mut env, config);
    }
    if let Some(port) = job.container_port {
        env.insert("CSFX_CONTAINER_PORT".to_string(), port.to_string());
    }
    env
}

pub fn builder_name(site_name: &str, deployment_id: Uuid) -> String {
    format!("build-{}-{}", site_name, short_id(deployment_id))
}

pub fn builder_request(config: &Config, job: &BuildJob) -> Value {
    let (cpu_millicores, memory_bytes, disk_bytes) = job.build_size.resources();
    json!({
        "name": builder_name(&job.site_name, job.deployment_id),
        "image": config.builder_image,
        "cpu_millicores": cpu_millicores,
        "memory_bytes": memory_bytes,
        "disk_bytes": disk_bytes,
        "env_vars": builder_environment(config, job),
        "resource_group_id": job.resource_group_id,
        "restart_policy": "never",
        "runtime_class": "firecracker",
    })
}

pub struct RuntimeSpec<'a> {
    pub site_name: &'a str,
    pub deployment_id: Uuid,
    pub resource_group_id: Uuid,
    pub image_ref: &'a str,
    pub container_port: u16,
    pub env: &'a BTreeMap<String, String>,
}

pub fn runtime_request(spec: &RuntimeSpec) -> Value {
    let mut env = spec.env.clone();
    env.insert("PORT".to_string(), spec.container_port.to_string());
    json!({
        "name": format!("site-{}-{}", spec.site_name, short_id(spec.deployment_id)),
        "image": spec.image_ref,
        "cpu_millicores": RUNTIME_CPU_MILLICORES,
        "memory_bytes": RUNTIME_MEMORY_BYTES,
        "disk_bytes": RUNTIME_DISK_BYTES,
        "env_vars": env,
        "ports": [{ "container_port": spec.container_port, "protocol": "tcp" }],
        "resource_group_id": spec.resource_group_id,
        "restart_policy": "always",
        "runtime_class": "firecracker",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn config(registry: Option<&str>) -> Config {
        Config {
            port: 1,
            edge_port: 2,
            callback_url: "http://builder:8007".to_string(),
            secret: "s".to_string(),
            builder_image: "builder:1".to_string(),
            registry: registry.map(str::to_string),
            registry_insecure: true,
            registry_mirror: Some("10.0.0.1:5050".to_string()),
            keep_deployments: 5,
            scheduler_url: String::new(),
            preview_domain: None,
            build_timeout: Duration::from_secs(1),
        }
    }

    fn job(mode: Mode) -> BuildJob {
        BuildJob {
            deployment_id: Uuid::from_u128(0xabcdef),
            site_id: Uuid::nil(),
            resource_group_id: Uuid::nil(),
            site_name: "shop".to_string(),
            mode,
            repo_url: "https://github.com/org/repo".to_string(),
            branch: "main".to_string(),
            root_dir: String::new(),
            framework: Some("vite".to_string()),
            install_command: Some("npm ci".to_string()),
            build_command: Some("npm run build".to_string()),
            output_dir: Some("dist".to_string()),
            start_command: None,
            container_port: Some(3000),
            build_size: BuildSize::default(),
            git_token: Some("tok".to_string()),
            env: BTreeMap::from([("A".to_string(), "1".to_string())]),
        }
    }

    #[test]
    fn container_mode_requires_a_registry_and_port() {
        assert!(job(Mode::Container).validate(&config(None)).is_err());
        assert!(job(Mode::Container)
            .validate(&config(Some("r:5000")))
            .is_ok());
        let mut portless = job(Mode::Container);
        portless.container_port = None;
        assert!(portless.validate(&config(Some("r:5000"))).is_err());
        assert!(job(Mode::Static).validate(&config(None)).is_ok());
    }

    #[test]
    fn build_sizes_scale_the_builder_resources() {
        let mut large = job(Mode::Static);
        large.build_size = BuildSize::Large;
        let request = builder_request(&config(None), &large);
        assert_eq!(request["cpu_millicores"], 8000);
        assert_eq!(request["memory_bytes"], 16 * GIB);
        let default = builder_request(&config(None), &job(Mode::Static));
        assert_eq!(default["cpu_millicores"], 2000);
        assert_eq!(default["memory_bytes"], 2 * GIB);
    }

    #[test]
    fn build_size_is_read_from_the_job_payload() {
        let sizes: Vec<BuildSize> = ["small", "medium", "large"]
            .iter()
            .map(|name| serde_json::from_value(serde_json::json!(name)).unwrap())
            .collect();
        assert_eq!(
            sizes,
            [BuildSize::Small, BuildSize::Medium, BuildSize::Large]
        );
        assert!(serde_json::from_value::<BuildSize>(serde_json::json!("huge")).is_err());
    }

    #[test]
    fn reserved_variable_names_are_rejected() {
        let mut reserved = job(Mode::Static);
        reserved
            .env
            .insert("CSFX_GIT_TOKEN".to_string(), "x".to_string());
        assert!(reserved.validate(&config(None)).is_err());
    }

    #[test]
    fn builder_request_carries_token_and_image_reference() {
        let request = builder_request(&config(Some("r:5000/")), &job(Mode::Container));
        let env = &request["env_vars"];
        assert_eq!(env["CSFX_MODE"], "container");
        assert_eq!(env["CSFX_GIT_TOKEN"], "tok");
        assert_eq!(
            env["CSFX_IMAGE_REF"],
            format!("r:5000/{}:{}", Uuid::nil(), "00000000")
        );
        assert_eq!(env["CSFX_REGISTRY_INSECURE"], "1");
        assert_eq!(env["CSFX_REGISTRY_MIRROR"], "10.0.0.1:5050");
        assert_eq!(request["restart_policy"], "never");
    }

    #[test]
    fn runtime_request_maps_the_container_port() {
        let env = BTreeMap::new();
        let request = runtime_request(&RuntimeSpec {
            site_name: "shop",
            deployment_id: Uuid::from_u128(1),
            resource_group_id: Uuid::nil(),
            image_ref: "img",
            container_port: 3000,
            env: &env,
        });
        assert_eq!(request["ports"][0]["container_port"], 3000);
        assert_eq!(request["env_vars"]["PORT"], "3000");
    }
}
