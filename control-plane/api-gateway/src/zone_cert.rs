use chrono::{Datelike, Duration, NaiveDate, Utc};
use entity::entities::system_settings;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, GeneralSubtree, IsCa, Issuer,
    KeyPair, KeyUsagePurpose, NameConstraints, SanType,
};
use sea_orm::{sea_query::OnConflict, ActiveValue, DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const VALIDITY_DAYS: i64 = 90;
const RENEW_BEFORE_DAYS: i64 = 30;
const PEM_CERTIFICATE_MARKER: &str = "-----BEGIN CERTIFICATE-----";

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

pub struct NameScope<'a> {
    pub zone: &'a str,
    pub excluded: &'a str,
}

pub fn needs_issue(stored: Option<&IssuedCertificate>, sans: &[String], today: NaiveDate) -> bool {
    match stored {
        Some(cert) => {
            cert.sans != sans
                || cert.expires_on - today < Duration::days(RENEW_BEFORE_DAYS)
                || !is_chained(cert)
        }
        None => true,
    }
}

fn is_chained(cert: &IssuedCertificate) -> bool {
    cert.cert_pem.matches(PEM_CERTIFICATE_MARKER).count() > 1
}

fn ymd(date: NaiveDate) -> (i32, u8, u8) {
    (date.year(), date.month() as u8, date.day() as u8)
}

fn validity(today: NaiveDate, params: &mut CertificateParams) -> NaiveDate {
    let expires_on = today + Duration::days(VALIDITY_DAYS);
    let (year, month, day) = ymd(today - Duration::days(1));
    params.not_before = rcgen::date_time_ymd(year, month, day);
    let (year, month, day) = ymd(expires_on);
    params.not_after = rcgen::date_time_ymd(year, month, day);
    expires_on
}

fn intermediate_params(scope: &NameScope, today: NaiveDate) -> CertificateParams {
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "csfx-gateway-intermediate");
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![GeneralSubtree::DnsName(scope.zone.to_string())],
        excluded_subtrees: vec![GeneralSubtree::DnsName(scope.excluded.to_string())],
    });
    validity(today, &mut params);
    params
}

pub fn issue(
    sans: &[String],
    scope: &NameScope,
    ca_cert_pem: &str,
    ca_key_pem: &str,
    today: NaiveDate,
) -> anyhow::Result<IssuedCertificate> {
    let root = Issuer::from_ca_cert_pem(ca_cert_pem, KeyPair::from_pem(ca_key_pem)?)?;
    let intermediate_key = KeyPair::generate()?;
    let intermediate_params = intermediate_params(scope, today);
    let intermediate = intermediate_params.signed_by(&intermediate_key, &root)?;
    let intermediate_issuer = Issuer::from_params(&intermediate_params, intermediate_key);

    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "csfx-gateway");
    let expires_on = validity(today, &mut params);
    params.subject_alt_names = sans
        .iter()
        .map(|san| san.as_str().try_into().map(SanType::DnsName))
        .collect::<Result<_, _>>()?;

    let key_pair = KeyPair::generate()?;
    let cert = params.signed_by(&key_pair, &intermediate_issuer)?;
    Ok(IssuedCertificate {
        sans: sans.to_vec(),
        cert_pem: format!("{}{}", cert.pem(), intermediate.pem()),
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
    let zone = crate::dns_sync::zone_from_env();
    let excluded = crate::dns_sync::platform_hostname(&zone);
    let scope = NameScope {
        zone: &zone,
        excluded: &excluded,
    };
    let issued = issue(sans, &scope, &ca_cert_pem, &ca_key_pem, today)?;
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

    fn scope() -> NameScope<'static> {
        NameScope {
            zone: "csfx.internal",
            excluded: "console.csfx.internal",
        }
    }

    fn sans() -> Vec<String> {
        vec!["web.csfx.internal".to_string()]
    }

    #[test]
    fn issued_certificate_is_signed_for_the_requested_names() {
        let (ca_cert, ca_key) = ca();
        let issued = issue(&sans(), &scope(), &ca_cert, &ca_key, today()).unwrap();
        assert!(issued.cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(issued.key_pem.contains("PRIVATE KEY"));
        assert_eq!(issued.sans, sans());
        assert_eq!(issued.expires_on, today() + Duration::days(VALIDITY_DAYS));
    }

    #[test]
    fn issued_certificate_carries_a_name_constrained_intermediate() {
        let (ca_cert, ca_key) = ca();
        let issued = issue(&sans(), &scope(), &ca_cert, &ca_key, today()).unwrap();
        assert_eq!(issued.cert_pem.matches(PEM_CERTIFICATE_MARKER).count(), 2);
        let pems: Vec<_> = pem_blocks(&issued.cert_pem);
        let (_, intermediate) = x509_parser::parse_x509_certificate(&pems[1]).unwrap();
        let constraints = intermediate
            .name_constraints()
            .unwrap()
            .expect("intermediate must be name constrained")
            .value;
        assert_eq!(constraints.permitted_subtrees.as_ref().unwrap().len(), 1);
        assert_eq!(constraints.excluded_subtrees.as_ref().unwrap().len(), 1);
        assert!(intermediate.is_ca());
    }

    #[test]
    fn unchained_certificates_are_reissued() {
        let (ca_cert, ca_key) = ca();
        let mut issued = issue(&sans(), &scope(), &ca_cert, &ca_key, today()).unwrap();
        issued.cert_pem = issued
            .cert_pem
            .split_inclusive("-----END CERTIFICATE-----\n")
            .next()
            .unwrap()
            .to_string();
        assert!(needs_issue(Some(&issued), &sans(), today()));
    }

    fn pem_blocks(pem: &str) -> Vec<Vec<u8>> {
        x509_parser::pem::Pem::iter_from_buffer(pem.as_bytes())
            .map(|block| block.unwrap().contents)
            .collect()
    }

    #[test]
    fn reissue_is_needed_when_missing_changed_or_close_to_expiry() {
        let (ca_cert, ca_key) = ca();
        let issued = issue(&sans(), &scope(), &ca_cert, &ca_key, today()).unwrap();
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
        let issued = issue(&sans(), &scope(), &ca_cert, &ca_key, today()).unwrap();
        let section = tls_section(&issued);
        assert_eq!(section["certificates"][0]["certFile"], issued.cert_pem);
        assert_eq!(section["certificates"][0]["keyFile"], issued.key_pem);
    }
}
