use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use std::path::Path;

const PRIVATE_KEY_FILE: &str = "/var/lib/csfx-agent/wg.key";

pub struct WgIdentity {
    pub private_key_b64: String,
    pub public_key_b64: String,
}

pub fn load_or_generate() -> Result<WgIdentity> {
    if Path::new(PRIVATE_KEY_FILE).exists() {
        let private_key_b64 = std::fs::read_to_string(PRIVATE_KEY_FILE)
            .context("Failed to read WireGuard private key")?
            .trim()
            .to_string();

        let public_key_b64 = public_key_from_private(&private_key_b64)?;

        tracing::info!("WireGuard: loaded existing identity keypair");
        return Ok(WgIdentity {
            private_key_b64,
            public_key_b64,
        });
    }

    generate()
}

fn generate() -> Result<WgIdentity> {
    let rng = SystemRandom::new();
    let mut private_bytes = [0u8; 32];
    rng.fill(&mut private_bytes)
        .map_err(|_| anyhow::anyhow!("failed to generate WireGuard key"))?;
    private_bytes[0] &= 248;
    private_bytes[31] &= 127;
    private_bytes[31] |= 64;

    let secret = x25519_dalek::StaticSecret::from(private_bytes);
    let public = x25519_dalek::PublicKey::from(&secret);

    let private_key_b64 = B64.encode(private_bytes);
    let public_key_b64 = B64.encode(public.to_bytes());

    std::fs::write(PRIVATE_KEY_FILE, &private_key_b64)
        .context("Failed to write WireGuard private key")?;
    set_permissions_600(PRIVATE_KEY_FILE)?;

    tracing::info!("WireGuard: generated new identity keypair");

    Ok(WgIdentity {
        private_key_b64,
        public_key_b64,
    })
}

fn public_key_from_private(private_key_b64: &str) -> Result<String> {
    let bytes = B64
        .decode(private_key_b64)
        .context("Failed to decode WireGuard private key")?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("WireGuard private key has invalid length"))?;

    let secret = x25519_dalek::StaticSecret::from(array);
    let public = x25519_dalek::PublicKey::from(&secret);
    Ok(B64.encode(public.to_bytes()))
}

#[cfg(unix)]
fn set_permissions_600(path: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms).context("Failed to set file permissions")
}

#[cfg(not(unix))]
fn set_permissions_600(_path: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_key_matches_rfc7748_vector() {
        let private = B64.encode(hex_bytes(
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
        ));
        let public = public_key_from_private(&private).unwrap();
        let expected = B64.encode(hex_bytes(
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a",
        ));
        assert_eq!(public, expected);
    }

    #[test]
    fn invalid_private_keys_are_rejected() {
        assert!(public_key_from_private("!!!").is_err());
        assert!(public_key_from_private(&B64.encode([0u8; 16])).is_err());
    }

    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }
}
