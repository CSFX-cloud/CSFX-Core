pub const SITE_IMAGE_HOST: &str = "sites.csfx.invalid";

pub fn uses_site_registry(image: &str) -> bool {
    let normalized = image.trim().to_ascii_lowercase();
    normalized
        .strip_prefix(SITE_IMAGE_HOST)
        .is_some_and(|rest| rest.starts_with('/') || rest.starts_with(':') || rest.is_empty())
}

const GROUP_NAMESPACE_PATH: &str = "/rg-";

pub const REGISTRY_ENDPOINT_VARIABLES: [&str; 2] =
    ["CSFX_SITE_REGISTRY", "CSFX_SITE_REGISTRY_ENDPOINT"];

pub fn configured_registry_endpoints() -> Vec<String> {
    REGISTRY_ENDPOINT_VARIABLES
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

pub fn mentions_site_repository(text: &str, endpoints: &[String]) -> bool {
    let lowered = text.to_ascii_lowercase();
    endpoints.iter().any(|endpoint| {
        lowered.match_indices(endpoint.as_str()).any(|(start, _)| {
            let before = lowered[..start].chars().next_back();
            let rest = &lowered[start + endpoint.len()..];
            let bounded_before = before.is_none_or(|c| !is_host_char(c));
            let bounded_after = rest
                .chars()
                .next()
                .is_none_or(|c| c == '/' || c == '@' || !is_host_char(c));
            bounded_before && bounded_after && !rest.starts_with(GROUP_NAMESPACE_PATH)
        })
    })
}

fn is_host_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':')
}

pub fn registry_prefix(resource_group_id: uuid::Uuid, registry_name: &str) -> String {
    format!("rg-{}/{registry_name}/", resource_group_id.simple())
}

pub fn repository_path(prefix: &str, relative: &str) -> Option<String> {
    let allowed = |c: char| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '/')
    };
    let valid = !relative.is_empty()
        && !relative.starts_with('/')
        && !relative.ends_with('/')
        && relative.chars().all(allowed)
        && relative
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    valid.then(|| format!("{prefix}{relative}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_image_hosts_are_detected_in_any_spelling() {
        assert!(uses_site_registry("sites.csfx.invalid/abc:1"));
        assert!(uses_site_registry("  SITES.CSFX.INVALID/abc"));
        assert!(uses_site_registry("sites.csfx.invalid:443/abc"));
    }

    #[test]
    fn registry_prefixes_use_the_group_and_registry_name() {
        let group = uuid::Uuid::from_u128(0xabc);
        assert_eq!(
            registry_prefix(group, "main"),
            "rg-00000000000000000000000000000abc/main/"
        );
    }

    #[test]
    fn repository_paths_stay_inside_the_prefix() {
        assert_eq!(
            repository_path("rg-1/main/", "app/web").as_deref(),
            Some("rg-1/main/app/web")
        );
        for bad in ["", "/x", "x/", "../other", "a/../b", "a//b", "UPPER", "a b"] {
            assert!(repository_path("rg-1/main/", bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn site_repositories_on_configured_endpoints_are_detected() {
        let endpoints = vec![
            "10.0.0.5:5060".to_string(),
            "registry.csfx.test".to_string(),
        ];
        assert!(mentions_site_repository("10.0.0.5:5060/abc:1", &endpoints));
        assert!(mentions_site_repository(
            "  REGISTRY.CSFX.TEST/abc",
            &endpoints
        ));
        assert!(mentions_site_repository(
            "services:\n  a:\n    image: 10.0.0.5:5060/x@sha256:ab",
            &endpoints
        ));
        assert!(!mentions_site_repository("110.0.0.5:5060/abc", &endpoints));
        assert!(!mentions_site_repository("10.0.0.5:50601/abc", &endpoints));
        assert!(!mentions_site_repository("nginx:1", &endpoints));
        assert!(!mentions_site_repository("10.0.0.5:5060/abc", &[]));
    }

    #[test]
    fn group_namespaces_on_configured_endpoints_stay_allowed() {
        let endpoints = vec!["10.0.0.5:5060".to_string()];
        assert!(!mentions_site_repository(
            "10.0.0.5:5060/rg-abc/main/app:1",
            &endpoints
        ));
    }

    #[test]
    fn other_images_are_not_site_images() {
        assert!(!uses_site_registry("nginx:1"));
        assert!(!uses_site_registry("ghcr.io/org/sites.csfx.invalid"));
        assert!(!uses_site_registry("sites.csfx.invalid.evil.com/x"));
    }
}
