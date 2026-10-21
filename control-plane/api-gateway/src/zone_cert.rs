use chrono::{Datelike, Duration, NaiveDate, Utc};
use entity::entities::system_settings;
use rcgen::{CertificateParams, DistinguishedName, DnType, Issuer, KeyPair, SanType};
use sea_orm::{sea_query::OnConflict, ActiveValue, DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const VALIDITY_DAYS: i64 = 90;
const RENEW_BEFORE_DAYS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuedCertificate {
    pub sans: Vec<String>,
    pub cert_pem: String,
    pub key_pem: String,
    pub expires_on: NaiveDate,
}

fn settings_key(gateway_id: Uuid) -> String {
    format!("ingress.certificate.{gateway_id}")
}

pub fn needs_issue(stored: Option<&IssuedCertificate>, sans: &[String], today: NaiveDate) -> bool {
    match stored {
        Some(cert) => {
            cert.sans != sans || cert.expires_on - today < Duration::days(RENEW_BEFORE_DAYS)
        }
        None => true,
    }
}

fn ymd(date: NaiveDate) -> (i32, u8, u8) {
    (date.year(), date.month() as u8, date.day() as u8)
}

pub fn issue(
    sans: &[String],
    ca_cert_pem: &str,
    ca_key_pem: &str,
    today: NaiveDate,
) -> anyhow::Result<IssuedCertificate> {
    let expires_on = today + Duration::days(VALIDITY_DAYS);
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "csfx-gateway");
    let (year, month, day) = ymd(today - Duration::days(1));
    params.not_before = rcgen::date_time_ymd(year, month, day);
    let (year, month, day) = ymd(expires_on);
    params.not_after = rcgen::date_time_ymd(year, month, day);
    params.subject_alt_names = sans
        .iter()
        .map(|san| san.as_str().try_into().map(SanType::DnsName))
        .collect::<Result<_, _>>()?;

    let key_pair = KeyPair::generate()?;
    let issuer = Issuer::from_ca_cert_pem(ca_cert_pem, KeyPair::from_pem(ca_key_pem)?)?;
    let cert = params.signed_by(&key_pair, &issuer)?;
    Ok(IssuedCertificate {
        sans: sans.to_vec(),
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
        expires_on,
    })
}

pub fn tls_section(cert: &IssuedCertificate) -> Value {
    json!({
        "certificates": [{ "certFile": cert.cert_pem, "keyFile": cert.key_pem }]
    })
}

pub async fn certificate_for(
    db: &DatabaseConnection,
    gateway_id: Uuid,
    sans: &[String],
) -> anyhow::Result<Option<IssuedCertificate>> {
    if sans.is_empty() {
        return Ok(None);
    }
    let today = Utc::now().date_naive();
    let stored = load(db, gateway_id).await?;
    if !needs_issue(stored.as_ref(), sans, today) {
        return Ok(stored);
    }
    let Some((ca_cert_pem, ca_key_pem)) = crate::tls::load_ca_pem().await else {
        tracing::warn!(gateway_id = %gateway_id, "no ca available, gateway serves its default certificate");
        return Ok(None);
    };
    let issued = issue(sans, &ca_cert_pem, &ca_key_pem, today)?;
    store(db, gateway_id, &issued).await?;
    tracing::info!(gateway_id = %gateway_id, sans = sans.len(), "gateway certificate issued");
    Ok(Some(issued))
}

async fn load(
    db: &DatabaseConnection,
    gateway_id: Uuid,
) -> anyhow::Result<Option<IssuedCertificate>> {
    let row = system_settings::Entity::find_by_id(settings_key(gateway_id))
        .one(db)
        .await?;
    Ok(row.and_then(|row| serde_json::from_value(row.value).ok()))
}

async fn store(
    db: &DatabaseConnection,
    gateway_id: Uuid,
    cert: &IssuedCertificate,
) -> anyhow::Result<()> {
    let row = system_settings::ActiveModel {
        key: ActiveValue::Set(settings_key(gateway_id)),
        value: ActiveValue::Set(serde_json::to_value(cert)?),
        updated_at: ActiveValue::Set(Utc::now().fixed_offset()),
    };
    system_settings::Entity::insert(row)
        .on_conflict(
            OnConflict::column(system_settings::Column::Key)
                .update_columns([
                    system_settings::Column::Value,
                    system_settings::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

pub async fn remove(db: &DatabaseConnection, gateway_id: Uuid) -> anyhow::Result<()> {
    system_settings::Entity::delete_by_id(settings_key(gateway_id))
        .exec(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{BasicConstraints, IsCa};

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 21).unwrap()
    }

    fn ca() -> (String, String) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let cert = params.self_signed(&key).unwrap();
        (cert.pem(), key.serialize_pem())
    }

    fn sans() -> Vec<String> {
        vec!["web.csfx.internal".to_string()]
    }

    #[test]
    fn issued_certificate_is_signed_for_the_requested_names() {
        let (ca_cert, ca_key) = ca();
        let issued = issue(&sans(), &ca_cert, &ca_key, today()).unwrap();
        assert!(issued.cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(issued.key_pem.contains("PRIVATE KEY"));
        assert_eq!(issued.sans, sans());
        assert_eq!(issued.expires_on, today() + Duration::days(VALIDITY_DAYS));
    }

    #[test]
    fn reissue_is_needed_when_missing_changed_or_close_to_expiry() {
        let (ca_cert, ca_key) = ca();
        let issued = issue(&sans(), &ca_cert, &ca_key, today()).unwrap();
        assert!(needs_issue(None, &sans(), today()));
        assert!(!needs_issue(Some(&issued), &sans(), today()));
        assert!(needs_issue(
            Some(&issued),
            &["other.csfx.internal".to_string()],
            today()
        ));
        let late = today() + Duration::days(VALIDITY_DAYS - RENEW_BEFORE_DAYS + 1);
        assert!(needs_issue(Some(&issued), &sans(), late));
    }

    #[test]
    fn tls_section_inlines_the_pem_pair() {
        let (ca_cert, ca_key) = ca();
        let issued = issue(&sans(), &ca_cert, &ca_key, today()).unwrap();
        let section = tls_section(&issued);
        assert_eq!(section["certificates"][0]["certFile"], issued.cert_pem);
        assert_eq!(section["certificates"][0]["keyFile"], issued.key_pem);
    }
}
