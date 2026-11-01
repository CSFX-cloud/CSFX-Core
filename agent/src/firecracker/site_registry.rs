use anyhow::{bail, Context, Result};
use oci_client::secrets::RegistryAuth;
use oci_client::Reference;

pub const SITE_IMAGE_HOST: &str = "sites.csfx.invalid";
const AGENT_USER: &str = "agent";
const SITE_REGISTRY_PORT: u16 = 5060;
const ENDPOINT_VARIABLE: &str = "CSFX_SITE_REGISTRY_ENDPOINT";
const DEFAULT_TAG: &str = "latest";

#[derive(Clone)]
pub struct SiteRegistry {
    endpoint: String,
    api_key: String,
}

pub fn endpoint_for(gateway_url: &str, explicit: Option<&str>) -> Option<String> {
    if let Some(endpoint) = explicit.filter(|value| !value.is_empty()) {
        return Some(endpoint.to_string());
    }
    let url = reqwest::Url::parse(gateway_url).ok()?;
    let host = url.host_str()?;
    Some(format!("{host}:{SITE_REGISTRY_PORT}"))
}

pub fn rewrite_host(reference: &Reference, endpoint: &str) -> Result<Reference> {
    let repository = reference.repository().to_string();
    if let Some(digest) = reference.digest() {
        return Ok(Reference::with_digest(
            endpoint.to_string(),
            repository,
            digest.to_string(),
        ));
    }
    let tag = reference.tag().unwrap_or(DEFAULT_TAG).to_string();
    Ok(Reference::with_tag(endpoint.to_string(), repository, tag))
}

impl SiteRegistry {
    pub fn from_gateway(gateway_url: &str, api_key: &str) -> Option<Self> {
        let explicit = std::env::var(ENDPOINT_VARIABLE).ok();
        Some(Self {
            endpoint: endpoint_for(gateway_url, explicit.as_deref())?,
            api_key: api_key.to_string(),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn auth(&self) -> RegistryAuth {
        RegistryAuth::Basic(AGENT_USER.to_string(), self.api_key.clone())
    }

    pub fn resolve(&self, reference: &Reference) -> Result<Reference> {
        if reference.registry() != SITE_IMAGE_HOST {
            bail!("reference does not point at the site image host");
        }
        rewrite_host(reference, &self.endpoint).context("Invalid site registry reference")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_defaults_to_the_gateway_host_on_the_registry_port() {
        assert_eq!(
            endpoint_for("https://10.0.0.5:8000", None).as_deref(),
            Some("10.0.0.5:5060")
        );
        assert_eq!(
            endpoint_for("https://localhost:8000", Some("")).as_deref(),
            Some("localhost:5060")
        );
        assert_eq!(
            endpoint_for("https://10.0.0.5:8000", Some("reg.internal:6000")).as_deref(),
            Some("reg.internal:6000")
        );
        assert!(endpoint_for("not a url", None).is_none());
    }

    #[test]
    fn site_images_are_rewritten_to_the_real_endpoint() {
        let registry = SiteRegistry {
            endpoint: "10.0.0.5:5060".to_string(),
            api_key: "key".to_string(),
        };
        let logical: Reference = "sites.csfx.invalid/abc:12345678".parse().unwrap();
        let real = registry.resolve(&logical).unwrap();
        assert_eq!(real.whole(), "10.0.0.5:5060/abc:12345678");
        let other: Reference = "ghcr.io/org/app:1".parse().unwrap();
        assert!(registry.resolve(&other).is_err());
    }
}
