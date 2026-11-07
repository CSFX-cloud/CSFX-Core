use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::signature::{UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};

const STORE_PREFIX: &str = "/nix/store/";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub arch: String,
    pub toplevel: String,
    pub bundle: String,
    pub released_at: String,
}

pub fn host_arch() -> String {
    format!("{}-linux", std::env::consts::ARCH)
}

fn decode_key(line: &str) -> Result<Vec<u8>> {
    let (_, key_base64) = line
        .split_once(':')
        .context("release public key must be name:base64")?;
    STANDARD
        .decode(key_base64)
        .context("release public key is not base64")
}

fn signed_by_any(manifest: &[u8], signatures: &[Vec<u8>], keys: &[Vec<u8>]) -> bool {
    keys.iter().any(|key| {
        let key = UnparsedPublicKey::new(&ED25519, key);
        signatures
            .iter()
            .any(|signature| key.verify(manifest, signature).is_ok())
    })
}

pub fn verify(manifest: &[u8], signatures: &str, public_keys: &str) -> Result<Manifest> {
    let keys = lines(public_keys)
        .map(decode_key)
        .collect::<Result<Vec<_>>>()?;
    let signatures = lines(signatures)
        .map(|line| {
            STANDARD
                .decode(line)
                .context("manifest signature is not base64")
        })
        .collect::<Result<Vec<_>>>()?;
    if !signed_by_any(manifest, &signatures, &keys) {
        bail!("manifest signature verification failed");
    }
    let parsed: Manifest =
        serde_json::from_slice(manifest).context("manifest is not valid json")?;
    validate(&parsed)?;
    Ok(parsed)
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty())
}

fn validate(manifest: &Manifest) -> Result<()> {
    let store_name = manifest.toplevel.strip_prefix(STORE_PREFIX);
    if !store_name.is_some_and(|name| !name.is_empty() && !name.contains('/')) {
        bail!(
            "manifest toplevel is not a store path: {}",
            manifest.toplevel
        );
    }
    if manifest.bundle.is_empty() || manifest.bundle.contains('/') {
        bail!(
            "manifest bundle must be a plain file name: {}",
            manifest.bundle
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn signing_material(body: &str) -> (String, String) {
        let pair = Ed25519KeyPair::from_seed_unchecked(&[7u8; 32]).unwrap();
        let signature = STANDARD.encode(pair.sign(body.as_bytes()).as_ref());
        let public = format!("test-1:{}", STANDARD.encode(pair.public_key().as_ref()));
        (signature, public)
    }

    fn body(toplevel: &str) -> String {
        format!(
            r#"{{"version":"0.3.0","arch":"x86_64-linux","toplevel":"{toplevel}","bundle":"b.tar.zst","released_at":"now"}}"#
        )
    }

    #[test]
    fn valid_signature_yields_manifest() {
        let body = body("/nix/store/abc-system");
        let (signature, public) = signing_material(&body);
        let manifest = verify(body.as_bytes(), &signature, &public).unwrap();
        assert_eq!(manifest.version, "0.3.0");
    }

    #[test]
    fn rotation_accepts_any_trusted_key_and_any_signature() {
        let body = body("/nix/store/abc-system");
        let (signature, public) = signing_material(&body);
        let other = format!("old-1:{}", STANDARD.encode([9u8; 32]));
        let keys = format!("{other}\n{public}\n");
        let signatures = format!("{}\n{signature}\n", STANDARD.encode([1u8; 64]));
        assert!(verify(body.as_bytes(), &signatures, &keys).is_ok());
        assert!(verify(body.as_bytes(), &signature, &other).is_err());
    }

    #[test]
    fn tampered_manifest_is_rejected() {
        let body = body("/nix/store/abc-system");
        let (signature, public) = signing_material(&body);
        let tampered = body.replace("0.3.0", "0.2.0");
        assert!(verify(tampered.as_bytes(), &signature, &public).is_err());
    }

    #[test]
    fn non_store_toplevel_is_rejected() {
        let body = body("/tmp/evil");
        let (signature, public) = signing_material(&body);
        assert!(verify(body.as_bytes(), &signature, &public).is_err());
    }
}
