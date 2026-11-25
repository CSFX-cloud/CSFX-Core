use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::hkdf::{self, KeyType};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

pub const KEY_VARIABLE: &str = "CSFX_ENV_ENCRYPTION_KEY";
pub const DERIVATION_SOURCE_VARIABLE: &str = "OBJECT_STORAGE_ENCRYPTION_KEY";
const DERIVATION_SALT: &[u8] = b"csfx-env-encryption";
const DERIVATION_INFO: &[u8] = b"workload-env-v1";
const KEY_LENGTH: usize = 32;
pub const MASK: &str = "********";
const PREFIX: &str = "enc:v1:";

const VAULT_PREFIX: &str = "vault://";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultRef {
    pub vault_id: Uuid,
    pub secret: String,
}

const MAX_SECRET_NAME_LENGTH: usize = 127;

pub fn is_valid_secret_name(name: &str) -> bool {
    (1..=MAX_SECRET_NAME_LENGTH).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn parse_vault_ref(value: &str) -> Option<VaultRef> {
    let (vault_id, secret) = value.strip_prefix(VAULT_PREFIX)?.split_once('/')?;
    is_valid_secret_name(secret).then_some(())?;
    Some(VaultRef {
        vault_id: Uuid::parse_str(vault_id).ok()?,
        secret: secret.to_string(),
    })
}

static KEY: OnceLock<LessSafeKey> = OnceLock::new();

#[derive(Debug, Error)]
pub enum EnvSecretError {
    #[error("{KEY_VARIABLE} or {DERIVATION_SOURCE_VARIABLE} must be set to 64 hex characters")]
    Key,
    #[error("environment payload could not be encrypted")]
    Encrypt,
    #[error("environment payload could not be decrypted")]
    Decrypt,
}

const BACKFILL_DELAY: Duration = Duration::from_secs(600);
const BACKFILL_INTERVAL: Duration = Duration::from_secs(3600);

pub fn spawn_backfill<F, Fut, E>(name: &'static str, run: F)
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = Result<u64, E>> + Send,
    E: Display,
{
    tokio::spawn(async move {
        tokio::time::sleep(BACKFILL_DELAY).await;
        loop {
            match run().await {
                Ok(0) => {}
                Ok(sealed) => tracing::info!(backfill = name, sealed, "legacy values encrypted"),
                Err(error) => {
                    tracing::warn!(backfill = name, error = %error, "legacy value encryption failed")
                }
            }
            tokio::time::sleep(BACKFILL_INTERVAL).await;
        }
    });
}

pub fn init() -> Result<(), EnvSecretError> {
    key().map(|_| ())
}

pub fn is_sealed(value: &Value) -> bool {
    value.as_str().is_some_and(|text| text.starts_with(PREFIX))
}

pub fn workload_env(id: Uuid) -> String {
    format!("workloads.env_vars:{id}")
}

pub fn build_env(id: Uuid) -> String {
    format!("site_deployments.build_env:{id}")
}

pub fn site_env(id: Uuid) -> String {
    format!("site_env_vars.value:{id}")
}

pub fn stack_source(id: Uuid) -> String {
    format!("workload_stacks.compose_source:{id}")
}

pub fn seal(value: &Value, context: &str) -> Result<Value, EnvSecretError> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| EnvSecretError::Encrypt)?;
    let mut buffer = serde_json::to_vec(value).map_err(|_| EnvSecretError::Encrypt)?;
    key()?
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(context.as_bytes()),
            &mut buffer,
        )
        .map_err(|_| EnvSecretError::Encrypt)?;
    let mut payload = nonce.to_vec();
    payload.extend_from_slice(&buffer);
    Ok(Value::String(format!(
        "{PREFIX}{}",
        crate::hex::encode(&payload)
    )))
}

pub fn open(value: &Value, context: &str) -> Result<Value, EnvSecretError> {
    let Some(encoded) = value.as_str().and_then(|text| text.strip_prefix(PREFIX)) else {
        return Ok(value.clone());
    };
    let payload = crate::hex::decode(encoded).ok_or(EnvSecretError::Decrypt)?;
    if payload.len() < NONCE_LEN {
        return Err(EnvSecretError::Decrypt);
    }
    let (nonce, sealed) = payload.split_at(NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| EnvSecretError::Decrypt)?;
    let mut buffer = sealed.to_vec();
    let plain = key()?
        .open_in_place(nonce, Aad::from(context.as_bytes()), &mut buffer)
        .map_err(|_| EnvSecretError::Decrypt)?;
    serde_json::from_slice(plain).map_err(|_| EnvSecretError::Decrypt)
}

pub fn seal_map(map: &HashMap<String, String>, context: &str) -> Result<Value, EnvSecretError> {
    seal(&serde_json::json!(map), context)
}

pub fn open_map(value: &Value, context: &str) -> Result<HashMap<String, String>, EnvSecretError> {
    match open(value, context)? {
        Value::Null => Ok(HashMap::new()),
        opened => serde_json::from_value(opened).map_err(|_| EnvSecretError::Decrypt),
    }
}

pub fn seal_text(text: &str, context: &str) -> Result<String, EnvSecretError> {
    match seal(&Value::String(text.to_string()), context)? {
        Value::String(sealed) => Ok(sealed),
        _ => Err(EnvSecretError::Encrypt),
    }
}

pub fn open_text(text: &str, context: &str) -> Result<String, EnvSecretError> {
    match open(&Value::String(text.to_string()), context)? {
        Value::String(opened) => Ok(opened),
        _ => Err(EnvSecretError::Decrypt),
    }
}

pub fn is_sealed_text(text: &str) -> bool {
    text.starts_with(PREFIX)
}

pub fn redact(stored: &Value, context: &str) -> Option<Value> {
    let opened = open_map(stored, context);
    if let Err(error) = &opened {
        tracing::error!(error = %error, "stored environment could not be read");
    }
    opened.ok().map(|map| serde_json::json!(masked(map)))
}

pub fn contains_mask(map: &HashMap<String, String>) -> bool {
    map.values().any(|value| value == MASK)
}

pub fn masked(map: HashMap<String, String>) -> HashMap<String, String> {
    map.into_iter()
        .map(|(name, value)| match parse_vault_ref(&value) {
            Some(_) => (name, value),
            None => (name, MASK.to_string()),
        })
        .collect()
}

pub fn restore_masked(
    submitted: HashMap<String, String>,
    stored: &HashMap<String, String>,
) -> HashMap<String, String> {
    submitted
        .into_iter()
        .filter_map(|(name, value)| match value == MASK {
            true => stored.get(&name).map(|kept| (name, kept.clone())),
            false => Some((name, value)),
        })
        .collect()
}

fn key() -> Result<&'static LessSafeKey, EnvSecretError> {
    if let Some(key) = KEY.get() {
        return Ok(key);
    }
    let bytes = key_material()?;
    let unbound = UnboundKey::new(&AES_256_GCM, &bytes).map_err(|_| EnvSecretError::Key)?;
    Ok(KEY.get_or_init(|| LessSafeKey::new(unbound)))
}

fn key_material() -> Result<Vec<u8>, EnvSecretError> {
    if let Ok(explicit) = std::env::var(KEY_VARIABLE) {
        return crate::hex::decode(&explicit).ok_or(EnvSecretError::Key);
    }
    let source = std::env::var(DERIVATION_SOURCE_VARIABLE).map_err(|_| EnvSecretError::Key)?;
    derive_key(&crate::hex::decode(&source).ok_or(EnvSecretError::Key)?)
}

struct DerivedLength;

impl KeyType for DerivedLength {
    fn len(&self) -> usize {
        KEY_LENGTH
    }
}

fn derive_key(source: &[u8]) -> Result<Vec<u8>, EnvSecretError> {
    let mut derived = vec![0u8; KEY_LENGTH];
    hkdf::Salt::new(hkdf::HKDF_SHA256, DERIVATION_SALT)
        .extract(source)
        .expand(&[DERIVATION_INFO], DerivedLength)
        .and_then(|okm| okm.fill(&mut derived))
        .map_err(|_| EnvSecretError::Key)?;
    Ok(derived)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXT: &str = "test:row-1";

    fn with_key() {
        unsafe { std::env::set_var(KEY_VARIABLE, "11".repeat(32)) };
    }

    #[test]
    fn sealed_value_hides_plaintext_and_roundtrips() {
        with_key();
        let map = HashMap::from([("API_KEY".to_string(), "hunter2".to_string())]);
        let sealed = seal_map(&map, CONTEXT).unwrap();
        assert!(is_sealed(&sealed));
        assert!(!sealed.to_string().contains("hunter2"));
        assert_eq!(open_map(&sealed, CONTEXT).unwrap(), map);
    }

    #[test]
    fn sealed_value_is_bound_to_its_row() {
        with_key();
        let sealed = seal(&serde_json::json!({ "A": "b" }), CONTEXT).unwrap();
        assert!(open(&sealed, "test:row-2").is_err());
    }

    #[test]
    fn legacy_plaintext_is_passed_through() {
        with_key();
        let legacy = serde_json::json!({ "A": "b" });
        assert_eq!(open(&legacy, CONTEXT).unwrap(), legacy);
    }

    #[test]
    fn redact_hides_every_value() {
        with_key();
        let sealed = seal(&serde_json::json!({ "A": "b" }), CONTEXT).unwrap();
        assert_eq!(
            redact(&sealed, CONTEXT).unwrap(),
            serde_json::json!({ "A": MASK })
        );
        assert_eq!(open_map(&Value::Null, CONTEXT).unwrap(), HashMap::new());
    }

    #[test]
    fn text_roundtrips_and_legacy_text_passes_through() {
        with_key();
        let sealed = seal_text("hunter2", CONTEXT).unwrap();
        assert!(is_sealed_text(&sealed));
        assert!(!sealed.contains("hunter2"));
        assert_eq!(open_text(&sealed, CONTEXT).unwrap(), "hunter2");
        assert_eq!(open_text("plain", CONTEXT).unwrap(), "plain");
    }

    #[test]
    fn derived_keys_are_stable_and_depend_on_the_source() {
        let first = derive_key(&[1u8; 32]).unwrap();
        assert_eq!(first.len(), KEY_LENGTH);
        assert_eq!(first, derive_key(&[1u8; 32]).unwrap());
        assert_ne!(first, derive_key(&[2u8; 32]).unwrap());
        assert_ne!(first, vec![1u8; 32]);
    }

    #[test]
    fn vault_references_parse_and_stay_visible_when_masked() {
        let id = Uuid::new_v4();
        let reference = format!("vault://{id}/DB_PASSWORD");
        assert_eq!(
            parse_vault_ref(&reference),
            Some(VaultRef {
                vault_id: id,
                secret: "DB_PASSWORD".to_string()
            })
        );
        for invalid in [
            "vault://",
            "vault://nope/X",
            &format!("vault://{id}/"),
            &format!("vault://{id}/../../other/X"),
            "plain",
        ] {
            assert!(parse_vault_ref(invalid).is_none());
        }
        let masked = masked(HashMap::from([
            ("A".to_string(), reference.clone()),
            ("B".to_string(), "plain".to_string()),
        ]));
        assert_eq!(masked["A"], reference);
        assert_eq!(masked["B"], MASK);
    }

    #[test]
    fn tampered_payload_is_rejected() {
        with_key();
        let sealed = seal(&serde_json::json!({ "A": "b" }), CONTEXT).unwrap();
        let text = sealed.as_str().unwrap();
        let flipped = format!("{}{}", &text[..text.len() - 1], "0");
        let flipped = if flipped == text {
            format!("{}1", &text[..text.len() - 1])
        } else {
            flipped
        };
        assert!(open(&Value::String(flipped), CONTEXT).is_err());
    }

    #[test]
    fn masked_values_are_restored_from_storage() {
        let stored = HashMap::from([("A".to_string(), "old".to_string())]);
        let submitted = HashMap::from([
            ("A".to_string(), MASK.to_string()),
            ("B".to_string(), "new".to_string()),
            ("C".to_string(), MASK.to_string()),
        ]);
        let restored = restore_masked(submitted, &stored);
        assert_eq!(restored["A"], "old");
        assert_eq!(restored["B"], "new");
        assert!(!restored.contains_key("C"));
    }
}
