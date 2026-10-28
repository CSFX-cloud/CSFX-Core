use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, path::Path};

const CREDENTIAL_GARAGE_ENV: &str = "garage-env";
const GARAGE_RPC_SECRET_KEY: &str = "GARAGE_RPC_SECRET";
const CREDENTIAL_PATRONI_SUPERUSER: &str = "patroni-superuser-password";
const CREDENTIAL_PATRONI_REPLICATION: &str = "patroni-replication-password";

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("environment variable {0} is not set")]
    MissingEnvironment(&'static str),
    #[error("DATABASE_URL does not contain a password")]
    MissingDatabasePassword,
    #[error("credentials directory is not available")]
    MissingCredentialsDirectory,
    #[error("credential {name} could not be read: {source}")]
    Credential {
        name: &'static str,
        source: std::io::Error,
    },
    #[error("cluster CA is not available")]
    MissingCa,
    #[error("garage rpc secret is missing from the garage environment file")]
    MissingGarageSecret,
}

const VIP_ADDRESS_VAR: &str = "CSFX_VIP_ADDRESS";
const VIP_PREFIX_VAR: &str = "CSFX_VIP_PREFIX";
const VIP_ROUTER_ID_VAR: &str = "CSFX_VIP_ROUTER_ID";
const DEFAULT_VIP_PREFIX: u8 = 24;
const DEFAULT_VIP_ROUTER_ID: u8 = 51;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterVip {
    pub address: String,
    pub prefix_length: u8,
    pub router_id: u8,
}

impl ClusterVip {
    pub fn validated(self) -> Result<Self, &'static str> {
        let usable = self
            .address
            .parse::<Ipv4Addr>()
            .is_ok_and(|ip| !(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast()));
        if !usable {
            return Err("address must be a unicast ipv4 address");
        }
        if !(8..=30).contains(&self.prefix_length) {
            return Err("prefix_length must be between 8 and 30");
        }
        if self.router_id == 0 {
            return Err("router_id must be between 1 and 255");
        }
        Ok(self)
    }

    pub fn from_env() -> Option<Self> {
        Self::from_values(
            std::env::var(VIP_ADDRESS_VAR).ok(),
            std::env::var(VIP_PREFIX_VAR).ok(),
            std::env::var(VIP_ROUTER_ID_VAR).ok(),
        )
    }

    pub fn from_values(
        address: Option<String>,
        prefix_length: Option<String>,
        router_id: Option<String>,
    ) -> Option<Self> {
        let address = address.filter(|value| !value.is_empty())?;
        Some(Self {
            address,
            prefix_length: parse_or(prefix_length, DEFAULT_VIP_PREFIX),
            router_id: parse_or(router_id, DEFAULT_VIP_ROUTER_ID),
        })
    }
}

fn parse_or(value: Option<String>, default: u8) -> u8 {
    value.and_then(|raw| raw.parse().ok()).unwrap_or(default)
}

#[derive(Debug, Serialize)]
pub struct ClusterSecrets {
    pub jwt_secret: String,
    pub garage_admin_token: String,
    pub object_storage_encryption_key: String,
    pub db_password: String,
    pub garage_rpc_secret: String,
    pub patroni_superuser_password: String,
    pub patroni_replication_password: String,
}

impl ClusterSecrets {
    pub fn load() -> Result<Self, BundleError> {
        let credentials = std::env::var("CREDENTIALS_DIRECTORY")
            .map_err(|_| BundleError::MissingCredentialsDirectory)?;
        let credentials = Path::new(&credentials);
        let database_url = environment("DATABASE_URL")?;

        Ok(Self {
            jwt_secret: environment("JWT_SECRET")?,
            garage_admin_token: environment("GARAGE_ADMIN_TOKEN")?,
            object_storage_encryption_key: environment("OBJECT_STORAGE_ENCRYPTION_KEY")?,
            db_password: database_password(&database_url)
                .ok_or(BundleError::MissingDatabasePassword)?
                .to_string(),
            garage_rpc_secret: env_file_value(
                &credential(credentials, CREDENTIAL_GARAGE_ENV)?,
                GARAGE_RPC_SECRET_KEY,
            )
            .ok_or(BundleError::MissingGarageSecret)?,
            patroni_superuser_password: credential(credentials, CREDENTIAL_PATRONI_SUPERUSER)?,
            patroni_replication_password: credential(credentials, CREDENTIAL_PATRONI_REPLICATION)?,
        })
    }
}

fn environment(name: &'static str) -> Result<String, BundleError> {
    std::env::var(name).map_err(|_| BundleError::MissingEnvironment(name))
}

fn credential(directory: &Path, name: &'static str) -> Result<String, BundleError> {
    std::fs::read_to_string(directory.join(name))
        .map(|value| value.trim().to_string())
        .map_err(|source| BundleError::Credential { name, source })
}

pub fn env_file_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key && !value.trim().is_empty()).then(|| value.trim().to_string())
    })
}

pub fn database_password(database_url: &str) -> Option<&str> {
    let (_, rest) = database_url.split_once("://")?;
    let (userinfo, _) = rest.rsplit_once('@')?;
    let (_, password) = userinfo.split_once(':')?;
    (!password.is_empty()).then_some(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vip(address: &str, prefix_length: u8, router_id: u8) -> ClusterVip {
        ClusterVip {
            address: address.into(),
            prefix_length,
            router_id,
        }
    }

    #[test]
    fn validation_accepts_a_unicast_address() {
        assert!(vip("192.168.1.50", 24, 51).validated().is_ok());
    }

    #[test]
    fn validation_rejects_unusable_values() {
        assert!(vip("not-an-ip", 24, 51).validated().is_err());
        assert!(vip("127.0.0.1", 24, 51).validated().is_err());
        assert!(vip("0.0.0.0", 24, 51).validated().is_err());
        assert!(vip("224.0.0.1", 24, 51).validated().is_err());
        assert!(vip("192.168.1.50", 31, 51).validated().is_err());
        assert!(vip("192.168.1.50", 24, 0).validated().is_err());
    }

    #[test]
    fn vip_defaults_apply_when_only_the_address_is_set() {
        let vip = ClusterVip::from_values(Some("192.168.1.50".into()), None, None).unwrap();
        assert_eq!(vip.prefix_length, 24);
        assert_eq!(vip.router_id, 51);
    }

    #[test]
    fn vip_is_absent_without_an_address() {
        assert!(ClusterVip::from_values(None, Some("16".into()), None).is_none());
        assert!(ClusterVip::from_values(Some(String::new()), None, None).is_none());
    }

    #[test]
    fn vip_uses_explicit_values() {
        let vip =
            ClusterVip::from_values(Some("10.0.0.5".into()), Some("16".into()), Some("7".into()))
                .unwrap();
        assert_eq!((vip.prefix_length, vip.router_id), (16, 7));
    }

    #[test]
    fn extracts_database_password() {
        assert_eq!(
            database_password("postgres://csfx:s3cret@localhost:5432/csfx"),
            Some("s3cret")
        );
    }

    #[test]
    fn password_may_contain_special_characters() {
        assert_eq!(
            database_password("postgres://csfx:a:b@c@127.0.0.1:5432/csfx"),
            Some("a:b@c")
        );
    }

    #[test]
    fn env_file_value_finds_the_key() {
        let contents = "GARAGE_ADMIN_TOKEN=admin\nGARAGE_RPC_SECRET=abc123\n";
        assert_eq!(
            env_file_value(contents, "GARAGE_RPC_SECRET"),
            Some("abc123".to_string())
        );
        assert_eq!(env_file_value(contents, "MISSING"), None);
        assert_eq!(
            env_file_value("GARAGE_RPC_SECRET=\n", "GARAGE_RPC_SECRET"),
            None
        );
    }

    #[test]
    fn missing_password_yields_none() {
        assert_eq!(database_password("postgres://csfx@localhost/csfx"), None);
        assert_eq!(database_password("postgres://csfx:@localhost/csfx"), None);
        assert_eq!(database_password("not a url"), None);
    }
}
