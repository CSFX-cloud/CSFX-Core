pub const SITE_IMAGE_HOST: &str = "sites.csfx.invalid";

pub fn uses_site_registry(image: &str) -> bool {
    let normalized = image.trim().to_ascii_lowercase();
    normalized
        .strip_prefix(SITE_IMAGE_HOST)
        .is_some_and(|rest| rest.starts_with('/') || rest.starts_with(':') || rest.is_empty())
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
    fn other_images_are_not_site_images() {
        assert!(!uses_site_registry("nginx:1"));
        assert!(!uses_site_registry("ghcr.io/org/sites.csfx.invalid"));
        assert!(!uses_site_registry("sites.csfx.invalid.evil.com/x"));
    }
}
