use ring::hmac;
use uuid::Uuid;

fn key(secret: &str) -> hmac::Key {
    hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn deployment_token(secret: &str, deployment_id: Uuid) -> String {
    to_hex(hmac::sign(&key(secret), deployment_id.as_bytes()).as_ref())
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).ok())
        .collect()
}

pub fn deployment_token_valid(secret: &str, deployment_id: Uuid, presented: &str) -> bool {
    from_hex(presented)
        .is_some_and(|tag| hmac::verify(&key(secret), deployment_id.as_bytes(), &tag).is_ok())
}

pub fn secret_valid(secret: &str, presented: &str) -> bool {
    const PROBE: &[u8] = b"csfx-site-secret";
    let tag = hmac::sign(&key(presented), PROBE);
    hmac::verify(&key(secret), PROBE, tag.as_ref()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_bound_to_their_deployment_and_secret() {
        let id = Uuid::from_u128(1);
        let token = deployment_token("s3cret", id);
        assert!(deployment_token_valid("s3cret", id, &token));
        assert!(!deployment_token_valid("other", id, &token));
        assert!(!deployment_token_valid(
            "s3cret",
            Uuid::from_u128(2),
            &token
        ));
        assert!(!deployment_token_valid("s3cret", id, ""));
    }

    #[test]
    fn shared_secret_comparison_rejects_mismatches() {
        assert!(secret_valid("abc", "abc"));
        assert!(!secret_valid("abc", "abd"));
        assert!(!secret_valid("abc", "ab"));
    }
}
