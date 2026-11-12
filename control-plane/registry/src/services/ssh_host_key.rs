const ALLOWED_KEY_TYPES: [&str; 5] = [
    "ssh-ed25519",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
    "ssh-rsa",
];
const MAX_KEY_BODY_BYTES: usize = 1024;

pub fn normalize(raw: &str) -> Option<String> {
    let mut fields = raw.split_whitespace();
    let (key_type, body) = (fields.next()?, fields.next()?);
    let body_valid = body.len() <= MAX_KEY_BODY_BYTES
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='));
    (ALLOWED_KEY_TYPES.contains(&key_type) && body_valid).then(|| format!("{key_type} {body}"))
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn valid_keys_are_reduced_to_type_and_body() {
        assert_eq!(
            normalize("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIabc+/= root@node\n").as_deref(),
            Some("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIabc+/=")
        );
    }

    #[test]
    fn extra_lines_never_survive_normalization() {
        let normalized = normalize("ssh-ed25519 AAAA\n@cert-authority * ssh-ed25519 BBBB").unwrap();
        assert_eq!(normalized, "ssh-ed25519 AAAA");
    }

    #[test]
    fn malformed_or_hostile_keys_are_rejected() {
        for bad in [
            "",
            "ssh-ed25519",
            "unknown-type AAAA",
            "ssh-ed25519 AA;AA",
            &format!("ssh-ed25519 {}", "A".repeat(2000)),
        ] {
            assert!(normalize(bad).is_none(), "{bad:?}");
        }
    }
}
