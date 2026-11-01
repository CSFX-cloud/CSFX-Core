pub const SITE_IMAGE_HOST: &str = "sites.csfx.invalid";

pub fn uses_site_registry(image: &str) -> bool {
    let normalized = image.trim().to_ascii_lowercase();
    normalized
        .strip_prefix(SITE_IMAGE_HOST)
        .is_some_and(|rest| rest.starts_with('/') || rest.starts_with(':') || rest.is_empty())
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
    fn other_images_are_not_site_images() {
        assert!(!uses_site_registry("nginx:1"));
        assert!(!uses_site_registry("ghcr.io/org/sites.csfx.invalid"));
        assert!(!uses_site_registry("sites.csfx.invalid.evil.com/x"));
    }
}
