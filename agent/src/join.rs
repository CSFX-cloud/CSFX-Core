use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};

const JOIN_PREFIX: &str = "csfx-join.";

#[derive(Debug, PartialEq)]
pub struct JoinTicket {
    pub gateway_url: String,
    pub token: String,
    pub ca_fingerprint: String,
    pub role: String,
}

impl JoinTicket {
    pub fn parse(value: &str) -> Result<Self> {
        let body = value
            .trim()
            .strip_prefix(JOIN_PREFIX)
            .context("join string must start with csfx-join.")?;
        let mut parts = body.splitn(4, '#');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(host), Some(token), Some(fingerprint), role)
                if !host.is_empty() && !token.is_empty() && !fingerprint.is_empty() =>
            {
                Ok(Self {
                    gateway_url: format!("https://{host}"),
                    token: token.to_string(),
                    ca_fingerprint: normalize_fingerprint(fingerprint),
                    role: parse_role(role)?,
                })
            }
            _ => bail!("malformed join string"),
        }
    }

    pub fn is_foreign_to(&self, ca_pem: Option<&str>, saved_gateway: Option<&str>) -> bool {
        match (ca_pem, saved_gateway) {
            (Some(pem), _) => self.verify_ca(pem).is_err(),
            (None, Some(gateway)) => gateway != self.gateway_url,
            (None, None) => false,
        }
    }

    pub fn verify_ca(&self, ca_pem: &str) -> Result<()> {
        let actual = ca_fingerprint(ca_pem)?;
        if actual != self.ca_fingerprint {
            bail!(
                "cluster CA fingerprint mismatch expected={} actual={}",
                self.ca_fingerprint,
                actual
            );
        }
        Ok(())
    }
}

fn parse_role(value: Option<&str>) -> Result<String> {
    match value {
        None | Some("worker") => Ok("worker".to_string()),
        Some("master") => Ok("master".to_string()),
        Some(other) => bail!("unknown role in join string role={other}"),
    }
}

fn normalize_fingerprint(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_hexdigit)
        .collect::<String>()
        .to_ascii_lowercase()
}

fn ca_fingerprint(pem: &str) -> Result<String> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    let der = STANDARD
        .decode(body)
        .context("cluster CA is not valid PEM")?;
    Ok(Sha256::digest(der)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEM: &str = "-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\n";
    const PEM_SHA256: &str = "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81";

    #[test]
    fn parses_ticket_and_normalizes_fingerprint() {
        let ticket =
            JoinTicket::parse("csfx-join.10.0.0.5:8000#csfx-bootstrap.abc#03:90:58:C6").unwrap();
        assert_eq!(ticket.gateway_url, "https://10.0.0.5:8000");
        assert_eq!(ticket.token, "csfx-bootstrap.abc");
        assert_eq!(ticket.ca_fingerprint, "039058c6");
        assert_eq!(ticket.role, "worker");
    }

    #[test]
    fn parses_explicit_role() {
        let master = JoinTicket::parse("csfx-join.h:1#t#aa#master").unwrap();
        assert_eq!(master.role, "master");
        assert!(JoinTicket::parse("csfx-join.h:1#t#aa#admin").is_err());
    }

    #[test]
    fn rejects_malformed_ticket() {
        assert!(JoinTicket::parse("10.0.0.5:8000#token#aa").is_err());
        assert!(JoinTicket::parse("csfx-join.10.0.0.5:8000#token").is_err());
    }

    #[test]
    fn detects_registration_of_another_cluster() {
        let other_pem = "-----BEGIN CERTIFICATE-----\nBAUG\n-----END CERTIFICATE-----\n";
        let ticket = JoinTicket::parse(&format!("csfx-join.h:1#t#{PEM_SHA256}")).unwrap();
        assert!(!ticket.is_foreign_to(Some(PEM), Some("https://localhost:8000")));
        assert!(ticket.is_foreign_to(Some(other_pem), None));
        assert!(ticket.is_foreign_to(None, Some("https://localhost:8000")));
        assert!(!ticket.is_foreign_to(None, Some("https://h:1")));
        assert!(!ticket.is_foreign_to(None, None));
    }

    #[test]
    fn verifies_matching_and_mismatching_ca() {
        let good = JoinTicket::parse(&format!("csfx-join.h:1#t#{PEM_SHA256}")).unwrap();
        assert!(good.verify_ca(PEM).is_ok());
        let bad = JoinTicket::parse("csfx-join.h:1#t#00").unwrap();
        assert!(bad.verify_ca(PEM).is_err());
    }
}
