use percent_encoding::percent_decode_str;
use tokio::process::Command;

use super::process::command_stream;
use crate::export::{engine::Exporter, ExportError};

const DEFAULT_PORT: &str = "5432";
const PG_DUMP_VARIABLE: &str = "CSFX_PG_DUMP";

#[derive(Debug, PartialEq)]
struct Connection {
    host: String,
    port: String,
    user: String,
    password: Option<String>,
    database: String,
    sslmode: Option<String>,
}

fn decode(value: &str) -> Result<String, ExportError> {
    percent_decode_str(value)
        .decode_utf8()
        .map(|decoded| decoded.into_owned())
        .map_err(|_| ExportError::InvalidDatabaseUrl)
}

fn parse(url: &str) -> Result<Connection, ExportError> {
    let rest = url
        .strip_prefix("postgres://")
        .or_else(|| url.strip_prefix("postgresql://"))
        .ok_or(ExportError::InvalidDatabaseUrl)?;
    let (authority, tail) = rest
        .split_once('/')
        .ok_or(ExportError::InvalidDatabaseUrl)?;
    let (credentials, hosts) = authority
        .rsplit_once('@')
        .ok_or(ExportError::InvalidDatabaseUrl)?;
    let (user, password) = match credentials.split_once(':') {
        Some((user, password)) => (decode(user)?, Some(decode(password)?)),
        None => (decode(credentials)?, None),
    };
    if hosts.contains(',') {
        return Err(ExportError::InvalidDatabaseUrl);
    }
    let (host, port) = match hosts.rsplit_once(':') {
        Some((host, port)) if !port.contains(']') => (host, port),
        _ => (hosts, DEFAULT_PORT),
    };
    let (database, query) = tail.split_once('?').unwrap_or((tail, ""));
    let sslmode = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("sslmode="))
        .map(str::to_string);
    Ok(Connection {
        host: host.trim_matches(['[', ']']).to_string(),
        port: port.to_string(),
        user,
        password,
        database: decode(database)?,
        sslmode,
    })
}

fn dump_command(connection: &Connection) -> Command {
    let binary = std::env::var(PG_DUMP_VARIABLE).unwrap_or_else(|_| "pg_dump".to_string());
    let mut command = Command::new(binary);
    command
        .args(["--format=custom", "--no-owner", "--no-acl"])
        .env("PGHOST", &connection.host)
        .env("PGPORT", &connection.port)
        .env("PGUSER", &connection.user)
        .env("PGDATABASE", &connection.database);
    if let Some(password) = &connection.password {
        command.env("PGPASSWORD", password);
    }
    if let Some(sslmode) = &connection.sslmode {
        command.env("PGSSLMODE", sslmode);
    }
    command
}

pub async fn export(exporter: &mut Exporter) -> Result<(), ExportError> {
    let url = std::env::var("DATABASE_URL").map_err(|_| ExportError::InvalidDatabaseUrl)?;
    let stream = command_stream(dump_command(&parse(&url)?), "pg_dump")?;
    exporter
        .add_stream(
            "database/postgres.dump".to_string(),
            "database",
            None,
            stream,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_urls_with_encoded_credentials() {
        let connection =
            parse("postgres://csfx:p%40ss%3Aword@db.internal:6432/csfx?sslmode=require").unwrap();
        assert_eq!(
            connection,
            Connection {
                host: "db.internal".to_string(),
                port: "6432".to_string(),
                user: "csfx".to_string(),
                password: Some("p@ss:word".to_string()),
                database: "csfx".to_string(),
                sslmode: Some("require".to_string()),
            }
        );
    }

    #[test]
    fn applies_default_port_and_handles_ipv6() {
        let connection = parse("postgresql://u:p@[::1]/db").unwrap();
        assert_eq!(connection.host, "::1");
        assert_eq!(connection.port, DEFAULT_PORT);
        assert_eq!(connection.sslmode, None);
    }

    #[test]
    fn rejects_unsupported_urls() {
        assert!(parse("mysql://u:p@h/db").is_err());
        assert!(parse("postgres://u:p@h1:1,h2:2/db").is_err());
        assert!(parse("postgres://u:p@h").is_err());
    }
}
