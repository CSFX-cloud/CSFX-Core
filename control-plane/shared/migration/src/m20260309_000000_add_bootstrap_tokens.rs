use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BootstrapTokens::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BootstrapTokens::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(BootstrapTokens::Token).string().not_null().unique_key())
                    .col(ColumnDef::new(BootstrapTokens::Description).string().null())
                    .col(ColumnDef::new(BootstrapTokens::CreatedBy).string().not_null())
                    .col(ColumnDef::new(BootstrapTokens::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(BootstrapTokens::ExpiresAt).date_time().not_null())
                    .col(ColumnDef::new(BootstrapTokens::MaxUses).integer().not_null())
                    .col(ColumnDef::new(BootstrapTokens::UseCount).integer().not_null().default(0))
                    .col(ColumnDef::new(BootstrapTokens::Revoked).boolean().not_null().default(false))
                    .col(ColumnDef::new(BootstrapTokens::RevokedAt).date_time().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BootstrapTokens::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum BootstrapTokens {
    Table,
    Id,
    Token,
    Description,
    CreatedBy,
    CreatedAt,
    ExpiresAt,
    MaxUses,
    UseCount,
    Revoked,
    RevokedAt,
}
