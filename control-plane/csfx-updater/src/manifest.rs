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

pub fn verify(manifest: &[u8], signature_base64: &str, public_key: &str) -> Result<Manifest> {
    let (_, key_base64) = public_key
        .trim()
        .split_once(':')
        .context("release public key must be name:base64")?;
    let key = STANDARD
        .decode(key_base64)
        .context("release public key is not base64")?;
    let signature = STANDARD
        .decode(signature_base64.trim())
        .context("manifest signature is not base64")?;
    UnparsedPublicKey::new(&ED25519, key)
        .verify(manifest, &signature)
        .map_err(|_| anyhow::anyhow!("manifest signature verification failed"))?;
    let parsed: Manifest =
        serde_json::from_slice(manifest).context("manifest is not valid json")?;
    validate(&parsed)?;
    Ok(parsed)
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
