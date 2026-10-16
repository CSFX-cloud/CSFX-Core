use serde::Serialize;
use std::path::Path;

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
