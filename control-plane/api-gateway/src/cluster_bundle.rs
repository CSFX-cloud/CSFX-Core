use serde::{Deserialize, Serialize};
use std::{net::Ipv4Addr, path::Path};

const CREDENTIAL_GARAGE_ENV: &str = "garage-env";
const GARAGE_RPC_SECRET_KEY: &str = "GARAGE_RPC_SECRET";
const CREDENTIAL_PATRONI_SUPERUSER: &str = "patroni-superuser-password";
const CREDENTIAL_PATRONI_REPLICATION: &str = "patroni-replication-password";
const CREDENTIAL_OPENBAO_SEAL_KEY: &str = "openbao-seal-key";
const CREDENTIAL_OPENBAO_ROLE_ID: &str = "openbao-role-id";
const CREDENTIAL_OPENBAO_SECRET_ID: &str = "openbao-secret-id";

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
    #[error("credential {0} is empty because openbao is not bootstrapped yet")]
    EmptyCredential(&'static str),
}

const SITE_SECRET_VAR: &str = "CSFX_SITE_SECRET";
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
    pub openbao_seal_key: String,
    pub openbao_role_id: String,
    pub openbao_secret_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_secret: Option<String>,
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
            openbao_seal_key: filled_credential(credentials, CREDENTIAL_OPENBAO_SEAL_KEY)?,
            openbao_role_id: filled_credential(credentials, CREDENTIAL_OPENBAO_ROLE_ID)?,
            openbao_secret_id: filled_credential(credentials, CREDENTIAL_OPENBAO_SECRET_ID)?,
            site_secret: std::env::var(SITE_SECRET_VAR)
                .ok()
                .filter(|value| !value.is_empty()),
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

fn filled_credential(directory: &Path, name: &'static str) -> Result<String, BundleError> {
    let value = credential(directory, name)?;
    if value.is_empty() {
        return Err(BundleError::EmptyCredential(name));
    }
    Ok(value)
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

    static ENVIRONMENT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const MANAGED_VARIABLES: [&str; 8] = [
        "CREDENTIALS_DIRECTORY",
        "DATABASE_URL",
        "JWT_SECRET",
        "GARAGE_ADMIN_TOKEN",
        "OBJECT_STORAGE_ENCRYPTION_KEY",
        SITE_SECRET_VAR,
        VIP_ADDRESS_VAR,
        VIP_PREFIX_VAR,
    ];

    fn credentials_directory() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!("csfx-bundle-{}", std::process::id()));
        std::fs::remove_dir_all(&directory).ok();
        std::fs::create_dir_all(&directory).unwrap();
        let files = [
            (CREDENTIAL_GARAGE_ENV, "GARAGE_RPC_SECRET=rpc-secret\n"),
            (CREDENTIAL_PATRONI_SUPERUSER, "super\n"),
            (CREDENTIAL_PATRONI_REPLICATION, "replica\n"),
            (CREDENTIAL_OPENBAO_SEAL_KEY, "seal\n"),
            (CREDENTIAL_OPENBAO_ROLE_ID, "role\n"),
            (CREDENTIAL_OPENBAO_SECRET_ID, "secret\n"),
        ];
        for (name, contents) in files {
            std::fs::write(directory.join(name), contents).unwrap();
        }
        directory
    }

    fn set_required_environment(directory: &Path) {
        std::env::set_var("CREDENTIALS_DIRECTORY", directory);
        std::env::set_var("DATABASE_URL", "postgres://csfx:db-pass@localhost/csfx");
        std::env::set_var("JWT_SECRET", "jwt");
        std::env::set_var("GARAGE_ADMIN_TOKEN", "admin");
        std::env::set_var("OBJECT_STORAGE_ENCRYPTION_KEY", "key");
        std::env::remove_var(SITE_SECRET_VAR);
    }

    #[test]
    fn cluster_secrets_are_assembled_from_environment_and_credentials() {
        let _guard = ENVIRONMENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved: Vec<_> = MANAGED_VARIABLES
            .iter()
            .map(|name| (*name, std::env::var(name).ok()))
            .collect();
        let directory = credentials_directory();

        for name in MANAGED_VARIABLES {
            std::env::remove_var(name);
        }
        assert!(matches!(
            ClusterSecrets::load(),
            Err(BundleError::MissingCredentialsDirectory)
        ));

        set_required_environment(&directory);
        std::env::remove_var("JWT_SECRET");
        assert!(matches!(
            ClusterSecrets::load(),
            Err(BundleError::MissingEnvironment("JWT_SECRET"))
        ));

        set_required_environment(&directory);
        std::env::set_var("DATABASE_URL", "postgres://csfx@localhost/csfx");
        assert!(matches!(
            ClusterSecrets::load(),
            Err(BundleError::MissingDatabasePassword)
        ));

        set_required_environment(&directory);
        let secrets = ClusterSecrets::load().unwrap();
        assert_eq!(secrets.db_password, "db-pass");
        assert_eq!(secrets.garage_rpc_secret, "rpc-secret");
        assert_eq!(secrets.patroni_superuser_password, "super");
        assert_eq!(secrets.openbao_secret_id, "secret");
        assert!(secrets.site_secret.is_none());

        std::env::set_var(SITE_SECRET_VAR, "site");
        assert_eq!(
            ClusterSecrets::load().unwrap().site_secret.as_deref(),
            Some("site")
        );

        std::fs::write(directory.join(CREDENTIAL_OPENBAO_ROLE_ID), "\n").unwrap();
        assert!(matches!(
            ClusterSecrets::load(),
            Err(BundleError::EmptyCredential(CREDENTIAL_OPENBAO_ROLE_ID))
        ));

        std::fs::write(directory.join(CREDENTIAL_GARAGE_ENV), "OTHER=1\n").unwrap();
        assert!(matches!(
            ClusterSecrets::load(),
            Err(BundleError::MissingGarageSecret)
        ));

        std::fs::remove_file(directory.join(CREDENTIAL_PATRONI_SUPERUSER)).unwrap();
        std::fs::write(
            directory.join(CREDENTIAL_GARAGE_ENV),
            "GARAGE_RPC_SECRET=x\n",
        )
        .unwrap();
        assert!(matches!(
            ClusterSecrets::load(),
            Err(BundleError::Credential {
                name: CREDENTIAL_PATRONI_SUPERUSER,
                ..
            })
        ));

        for (name, value) in saved {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn vip_is_read_from_the_environment() {
        let _guard = ENVIRONMENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved: Vec<_> = [VIP_ADDRESS_VAR, VIP_PREFIX_VAR, VIP_ROUTER_ID_VAR]
            .iter()
            .map(|name| (*name, std::env::var(name).ok()))
            .collect();
        std::env::remove_var(VIP_ADDRESS_VAR);
        assert!(ClusterVip::from_env().is_none());
        std::env::set_var(VIP_ADDRESS_VAR, "10.1.2.3");
        std::env::set_var(VIP_PREFIX_VAR, "20");
        std::env::remove_var(VIP_ROUTER_ID_VAR);
        let vip = ClusterVip::from_env().unwrap();
        assert_eq!(
            (vip.address.as_str(), vip.prefix_length, vip.router_id),
            ("10.1.2.3", 20, 51)
        );
        for (name, value) in saved {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    #[test]
    fn bundle_errors_render_readable_messages() {
        assert_eq!(
            BundleError::MissingEnvironment("JWT_SECRET").to_string(),
            "environment variable JWT_SECRET is not set"
        );
        assert_eq!(
            BundleError::EmptyCredential("x").to_string(),
            "credential x is empty because openbao is not bootstrapped yet"
        );
        assert_eq!(
            BundleError::MissingCa.to_string(),
            "cluster CA is not available"
        );
    }
}
