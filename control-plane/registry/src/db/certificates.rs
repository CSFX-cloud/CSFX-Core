use anyhow::Result;
use entity::{agent_certificates, certificate_revocations};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub async fn create_certificate(
    db: &DatabaseConnection,
    agent_id: Uuid,
    serial_number: i64,
    certificate_pem: String,
    public_key_pem: String,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<agent_certificates::Model> {
    let model = agent_certificates::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        serial_number: Set(serial_number),
        certificate_pem: Set(certificate_pem),
        public_key_pem: Set(public_key_pem),
        issued_at: Set(chrono::Utc::now().naive_utc()),
        expires_at: Set(expires_at.naive_utc()),
        is_active: Set(true),
    };

    Ok(model.insert(db).await?)
}

pub async fn get_active_certificate(
    db: &DatabaseConnection,
    agent_id: Uuid,
) -> Result<Option<agent_certificates::Model>> {
    Ok(agent_certificates::Entity::find()
        .filter(agent_certificates::Column::AgentId.eq(agent_id))
        .filter(agent_certificates::Column::IsActive.eq(true))
        .one(db)
        .await?)
}

pub async fn deactivate_certificates(db: &DatabaseConnection, agent_id: Uuid) -> Result<u64> {
    let result = agent_certificates::Entity::update_many()
        .col_expr(
            agent_certificates::Column::IsActive,
            sea_orm::sea_query::Expr::value(false),
        )
        .filter(agent_certificates::Column::AgentId.eq(agent_id))
        .filter(agent_certificates::Column::IsActive.eq(true))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}

pub async fn revoke_certificate(
    db: &DatabaseConnection,
    serial_number: i64,
    agent_id: Uuid,
    reason: String,
) -> Result<()> {
    let revocation = certificate_revocations::ActiveModel {
        id: Set(Uuid::new_v4()),
        serial_number: Set(serial_number),
        agent_id: Set(agent_id),
        revoked_at: Set(chrono::Utc::now().naive_utc()),
        reason: Set(reason),
    };

    revocation.insert(db).await?;

    agent_certificates::Entity::update_many()
        .col_expr(
            agent_certificates::Column::IsActive,
            sea_orm::sea_query::Expr::value(false),
        )
        .filter(agent_certificates::Column::SerialNumber.eq(serial_number))
        .exec(db)
        .await?;

    Ok(())
}

pub async fn get_revoked_serials(db: &DatabaseConnection) -> Result<Vec<i64>> {
    Ok(certificate_revocations::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|r| r.serial_number)
        .collect())
}

pub async fn is_revoked(db: &DatabaseConnection, serial_number: i64) -> Result<bool> {
    Ok(certificate_revocations::Entity::find()
        .filter(certificate_revocations::Column::SerialNumber.eq(serial_number))
        .one(db)
        .await?
        .is_some())
}

pub async fn verify_client_cert(
    db: &DatabaseConnection,
    agent_id: Uuid,
    cert_pem: &str,
) -> Result<bool> {
    let cert = agent_certificates::Entity::find()
        .filter(agent_certificates::Column::AgentId.eq(agent_id))
        .filter(agent_certificates::Column::IsActive.eq(true))
        .filter(agent_certificates::Column::CertificatePem.eq(cert_pem))
        .one(db)
        .await?;

    match cert {
        None => Ok(false),
        Some(c) => {
            let revoked = is_revoked(db, c.serial_number).await?;
            Ok(!revoked)
        }
    }
}
