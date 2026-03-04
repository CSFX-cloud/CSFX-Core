use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column(text_null(Agents::PublicKeyPem))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AgentCertificates::Table)
                    .if_not_exists()
                    .col(pk_uuid(AgentCertificates::Id))
                    .col(uuid(AgentCertificates::AgentId))
                    .col(big_integer(AgentCertificates::SerialNumber))
                    .col(text(AgentCertificates::CertificatePem))
                    .col(text(AgentCertificates::PublicKeyPem))
                    .col(timestamp(AgentCertificates::IssuedAt))
                    .col(timestamp(AgentCertificates::ExpiresAt))
                    .col(boolean(AgentCertificates::IsActive).default(true))
                    .foreign_key(
                        ForeignKey::create()
                            .from(AgentCertificates::Table, AgentCertificates::AgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CertificateRevocations::Table)
                    .if_not_exists()
                    .col(pk_uuid(CertificateRevocations::Id))
                    .col(big_integer(CertificateRevocations::SerialNumber))
                    .col(uuid(CertificateRevocations::AgentId))
                    .col(timestamp(CertificateRevocations::RevokedAt))
                    .col(string(CertificateRevocations::Reason))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_agent_certificates_agent_id")
                    .table(AgentCertificates::Table)
                    .col(AgentCertificates::AgentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_agent_certificates_serial")
                    .table(AgentCertificates::Table)
                    .col(AgentCertificates::SerialNumber)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_certificate_revocations_serial")
                    .table(CertificateRevocations::Table)
                    .col(CertificateRevocations::SerialNumber)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_certificate_revocations_serial")
                    .table(CertificateRevocations::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_agent_certificates_serial")
                    .table(AgentCertificates::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_agent_certificates_agent_id")
                    .table(AgentCertificates::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(CertificateRevocations::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(AgentCertificates::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .drop_column(Agents::PublicKeyPem)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
    PublicKeyPem,
}

#[derive(DeriveIden)]
enum AgentCertificates {
    Table,
    Id,
    AgentId,
    SerialNumber,
    CertificatePem,
    PublicKeyPem,
    IssuedAt,
    ExpiresAt,
    IsActive,
}

#[derive(DeriveIden)]
enum CertificateRevocations {
    Table,
    Id,
    SerialNumber,
    AgentId,
    RevokedAt,
    Reason,
}
