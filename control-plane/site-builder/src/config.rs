use anyhow::{Context, Result};
use std::time::Duration;

const DEFAULT_PORT: u16 = 8007;
const DEFAULT_EDGE_PORT: u16 = 8008;
const DEFAULT_BUILD_TIMEOUT_SECS: u64 = 900;
const DEFAULT_BUILDER_IMAGE: &str = "ghcr.io/csfx-cloud/csfx-site-builder:latest";
const DEFAULT_SCHEDULER_URL: &str = "http://localhost:8002";

pub struct Config {
    pub port: u16,
    pub edge_port: u16,
    pub callback_url: String,
    pub secret: String,
    pub builder_image: String,
    pub registry: Option<String>,
    pub scheduler_url: String,
    pub preview_domain: Option<String>,
    pub build_timeout: Duration,
}

fn optional(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn parsed<T: std::str::FromStr>(name: &str, default: T) -> Result<T> {
    match optional(name) {
        Some(value) => value
            .parse()
            .map_err(|_| anyhow::anyhow!("{name} is not valid")),
        None => Ok(default),
    }
}

impl Config {
    pub fn from_environment() -> Result<Self> {
        Ok(Self {
            port: parsed("SITE_BUILDER_PORT", DEFAULT_PORT)?,
            edge_port: parsed("SITE_EDGE_PORT", DEFAULT_EDGE_PORT)?,
            callback_url: optional("CSFX_SITE_CALLBACK_URL")
                .context("CSFX_SITE_CALLBACK_URL must be set")?
                .trim_end_matches('/')
                .to_string(),
            secret: optional("CSFX_SITE_SECRET").context("CSFX_SITE_SECRET must be set")?,
            builder_image: optional("CSFX_SITE_BUILDER_IMAGE")
                .unwrap_or_else(|| DEFAULT_BUILDER_IMAGE.to_string()),
            registry: optional("CSFX_SITE_REGISTRY"),
            scheduler_url: optional("SCHEDULER_SERVICE_URL")
                .unwrap_or_else(|| DEFAULT_SCHEDULER_URL.to_string()),
            preview_domain: optional("CSFX_SITE_PREVIEW_DOMAIN"),
            build_timeout: Duration::from_secs(parsed(
                "CSFX_SITE_BUILD_TIMEOUT_SECS",
                DEFAULT_BUILD_TIMEOUT_SECS,
            )?),
        })
    }
}
