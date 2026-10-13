use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MIN_PIN_LENGTH: usize = 8;
const DEFAULT_PIN_PATH: &str = "/var/lib/csfx-cp/console-pin.json";
const DEFAULT_LOCK_PATH: &str = "/var/lib/csfx-cp/console-pin-lock.json";
const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const RECOVERY_GROUPS: usize = 5;
const RECOVERY_GROUP_LENGTH: usize = 4;
const FREE_ATTEMPTS: u32 = 5;
const BASE_LOCK_SECONDS: u64 = 60;
const MAX_LOCK_SECONDS: u64 = 3600;

#[derive(Debug, Error)]
pub enum PinError {
    #[error("console pin not provisioned")]
    NotProvisioned,
    #[error("console pin locked seconds={0}")]
    Locked(u64),
    #[error("console pin rejected")]
    Rejected,
    #[error("console pin shorter than {MIN_PIN_LENGTH} characters")]
    Weak,
    #[error("console pin io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("console pin serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("console pin hashing error: {0}")]
    Hashing(#[from] bcrypt::BcryptError),
}

#[derive(Serialize, Deserialize)]
struct StoredPin {
    pin_hash: String,
    recovery_hash: String,
}

#[derive(Serialize, Deserialize, Default)]
struct LockState {
    failures: u32,
    locked_until: u64,
}

fn pin_path() -> PathBuf {
    std::env::var("CSFX_CONSOLE_PIN_FILE")
        .unwrap_or_else(|_| DEFAULT_PIN_PATH.to_string())
        .into()
}

fn lock_path() -> PathBuf {
    std::env::var("CSFX_CONSOLE_PIN_LOCK_FILE")
        .unwrap_or_else(|_| DEFAULT_LOCK_PATH.to_string())
        .into()
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

fn write_private(path: &PathBuf, content: &[u8]) -> Result<(), PinError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content)?;
    Ok(())
}

fn generate_recovery_key() -> String {
    let groups: Vec<String> = (0..RECOVERY_GROUPS)
        .map(|_| {
            (0..RECOVERY_GROUP_LENGTH)
                .map(|_| RECOVERY_ALPHABET[rand::random_range(0..RECOVERY_ALPHABET.len())] as char)
                .collect()
        })
        .collect();
    groups.join("-")
}

fn normalize_recovery_key(candidate: &str) -> String {
    candidate
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

pub fn is_provisioned() -> bool {
    pin_path().exists()
}

pub fn provision(pin: &str) -> Result<String, PinError> {
    if pin.chars().count() < MIN_PIN_LENGTH {
        return Err(PinError::Weak);
    }
    let recovery_key = generate_recovery_key();
    let stored = StoredPin {
        pin_hash: bcrypt::hash(pin, bcrypt::DEFAULT_COST)?,
        recovery_hash: bcrypt::hash(normalize_recovery_key(&recovery_key), bcrypt::DEFAULT_COST)?,
    };
    write_private(&pin_path(), &serde_json::to_vec(&stored)?)?;
    write_private(&lock_path(), &serde_json::to_vec(&LockState::default())?)?;
    Ok(recovery_key)
}

fn load_stored() -> Result<StoredPin, PinError> {
    match std::fs::read(pin_path()) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(PinError::NotProvisioned),
        Err(err) => Err(err.into()),
    }
}

fn load_lock() -> LockState {
    std::fs::read(lock_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_lock(state: &LockState) -> Result<(), PinError> {
    write_private(&lock_path(), &serde_json::to_vec(state)?)
}

fn lock_duration(failures: u32) -> u64 {
    let doublings = failures.saturating_sub(FREE_ATTEMPTS).min(16);
    BASE_LOCK_SECONDS
        .saturating_mul(1u64 << doublings)
        .min(MAX_LOCK_SECONDS)
}

fn matches_stored(stored: &StoredPin, candidate: &str) -> Result<bool, PinError> {
    let pin_ok = bcrypt::verify(candidate, &stored.pin_hash)?;
    let recovery_ok = bcrypt::verify(normalize_recovery_key(candidate), &stored.recovery_hash)?;
    Ok(pin_ok || recovery_ok)
}

pub fn verify(candidate: &str) -> Result<(), PinError> {
    let stored = load_stored()?;
    let mut state = load_lock();
    let now = now_seconds();
    if state.locked_until > now {
        return Err(PinError::Locked(state.locked_until - now));
    }
    if matches_stored(&stored, candidate)? {
        save_lock(&LockState::default())?;
        return Ok(());
    }
    state.failures += 1;
    if state.failures >= FREE_ATTEMPTS {
        state.locked_until = now + lock_duration(state.failures);
    }
    save_lock(&state)?;
    tracing::warn!(event = "console.pin_failed", failures = state.failures);
    Err(PinError::Rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_duration_grows_and_caps() {
        assert_eq!(lock_duration(5), 60);
        assert_eq!(lock_duration(6), 120);
        assert_eq!(lock_duration(40), MAX_LOCK_SECONDS);
    }

    #[test]
    fn recovery_key_has_expected_shape() {
        let key = generate_recovery_key();
        assert_eq!(
            key.len(),
            RECOVERY_GROUPS * RECOVERY_GROUP_LENGTH + RECOVERY_GROUPS - 1
        );
        assert_eq!(
            normalize_recovery_key(&key.to_lowercase()),
            normalize_recovery_key(&key)
        );
    }

    #[test]
    fn provisioned_pin_verifies_and_locks_after_repeated_failures() {
        let dir = std::env::temp_dir().join(format!("csfx-pin-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("CSFX_CONSOLE_PIN_FILE", dir.join("pin.json"));
        std::env::set_var("CSFX_CONSOLE_PIN_LOCK_FILE", dir.join("lock.json"));

        assert!(matches!(verify("whatever"), Err(PinError::NotProvisioned)));
        let recovery_key = provision("correct-horse").unwrap();
        assert!(verify("correct-horse").is_ok());
        assert!(verify(&recovery_key.to_lowercase()).is_ok());
        for _ in 0..FREE_ATTEMPTS {
            assert!(matches!(verify("wrong-pin-1"), Err(PinError::Rejected)));
        }
        assert!(matches!(verify("correct-horse"), Err(PinError::Locked(_))));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
