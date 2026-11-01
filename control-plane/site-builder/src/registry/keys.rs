use crate::error::ServiceError;
use chrono::Utc;
use entity::{site_registry_keys, SiteRegistryKeys};
use jsonwebtoken::EncodingKey;
use rcgen::{date_time_ymd, CertificateParams, KeyPair, PKCS_RSA_SHA256};
use ring::digest;
use rsa::{
    pkcs8::{EncodePrivateKey, LineEnding},
    rand_core::OsRng,
    RsaPrivateKey,
};
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait, Set};

const RSA_BITS: usize = 2048;
const KEY_ROW_ID: i16 = 1;
const KEY_ID_BYTES: usize = 8;
const CERTIFICATE_NAME: &str = "csfx-site-registry";

pub struct RegistryKeys {
    pub encoding: EncodingKey,
    pub certificate_pem: String,
    pub key_id: String,
}

fn fingerprint(certificate_pem: &str) -> String {
    digest::digest(&digest::SHA256, certificate_pem.as_bytes())
        .as_ref()
        .iter()
        .take(KEY_ID_BYTES)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn crypto(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Crypto(error.to_string())
}

pub fn generate() -> Result<(String, String), ServiceError> {
    let private_key = RsaPrivateKey::new(&mut OsRng, RSA_BITS).map_err(crypto)?;
    let private_pem = private_key.to_pkcs8_pem(LineEnding::LF).map_err(crypto)?;
    let key_pair =
        KeyPair::from_pem_and_sign_algo(&private_pem, &PKCS_RSA_SHA256).map_err(crypto)?;
    let mut params = CertificateParams::new(vec![CERTIFICATE_NAME.to_string()]).map_err(crypto)?;
    params.not_before = date_time_ymd(2025, 1, 1);
    params.not_after = date_time_ymd(2125, 1, 1);
    let certificate = params.self_signed(&key_pair).map_err(crypto)?;
    Ok((key_pair.serialize_pem(), certificate.pem()))
}

fn from_row(row: site_registry_keys::Model) -> Result<RegistryKeys, ServiceError> {
    Ok(RegistryKeys {
        encoding: EncodingKey::from_rsa_pem(row.private_key_pem.as_bytes()).map_err(crypto)?,
        key_id: fingerprint(&row.certificate_pem),
        certificate_pem: row.certificate_pem,
    })
}

pub async fn load_or_create(db: &DatabaseConnection) -> Result<RegistryKeys, ServiceError> {
    if let Some(row) = SiteRegistryKeys::find_by_id(KEY_ROW_ID).one(db).await? {
        return from_row(row);
    }
    let (private_key_pem, certificate_pem) = generate()?;
    let row = site_registry_keys::ActiveModel {
        id: Set(KEY_ROW_ID),
        private_key_pem: Set(private_key_pem),
        certificate_pem: Set(certificate_pem),
        created_at: Set(Utc::now().naive_utc()),
    };
    SiteRegistryKeys::insert(row)
        .on_conflict(
            OnConflict::column(site_registry_keys::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await?;
    let stored = SiteRegistryKeys::find_by_id(KEY_ROW_ID)
        .one(db)
        .await?
        .ok_or_else(|| crypto("signing key was not stored"))?;
    from_row(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_material_is_usable_for_signing() {
        let (key, certificate) = generate().unwrap();
        assert!(key.contains("PRIVATE KEY"));
        assert!(certificate.contains("BEGIN CERTIFICATE"));
        assert!(EncodingKey::from_rsa_pem(key.as_bytes()).is_ok());
    }
}
