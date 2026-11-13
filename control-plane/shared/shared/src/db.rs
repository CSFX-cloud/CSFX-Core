use sea_orm::{ConnectOptions, Database, DbConn, DbErr};
use std::env;
use std::str::FromStr;
use std::time::Duration;

const DEFAULT_MAX_CONNECTIONS: u32 = 4;
const DEFAULT_MIN_CONNECTIONS: u32 = 1;
const DEFAULT_ACQUIRE_TIMEOUT_SECS: u64 = 5;

pub async fn establish_connection() -> Result<DbConn, DbErr> {
    establish_pool("", DEFAULT_MAX_CONNECTIONS).await
}

pub async fn establish_pool(name: &str, default_max_connections: u32) -> Result<DbConn, DbErr> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let prefix = variable_prefix(name);
    let max_connections = env_or(
        &format!("{prefix}_MAX_CONNECTIONS"),
        default_max_connections,
    )?;
    let min_connections = env_or(
        &format!("{prefix}_MIN_CONNECTIONS"),
        DEFAULT_MIN_CONNECTIONS,
    )?
    .min(max_connections);
    let acquire_timeout = env_or(
        &format!("{prefix}_ACQUIRE_TIMEOUT_SECS"),
        DEFAULT_ACQUIRE_TIMEOUT_SECS,
    )?;

    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout));

    let db_conn = Database::connect(options).await?;
    tracing::info!(
        pool = name,
        max_connections,
        min_connections,
        acquire_timeout_secs = acquire_timeout,
        "database connection established"
    );
    Ok(db_conn)
}

fn variable_prefix(name: &str) -> String {
    match name {
        "" => "CSFX_DB".to_string(),
        name => format!("CSFX_DB_{}", name.to_uppercase()),
    }
}

fn env_or<T: FromStr>(name: &str, default: T) -> Result<T, DbErr> {
    crate::env::parsed(name, default).map_err(DbErr::Custom)
}

#[cfg(test)]
mod tests {
    use super::variable_prefix;

    #[test]
    fn pool_names_select_their_own_variables() {
        assert_eq!(variable_prefix(""), "CSFX_DB");
        assert_eq!(variable_prefix("agent"), "CSFX_DB_AGENT");
    }
}
