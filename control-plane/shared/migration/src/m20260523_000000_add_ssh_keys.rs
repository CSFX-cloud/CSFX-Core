use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserSshKeys::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UserSshKeys::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(UserSshKeys::UserId).uuid().not_null())
                    .col(ColumnDef::new(UserSshKeys::Name).string().not_null())
                    .col(ColumnDef::new(UserSshKeys::PublicKey).text().not_null())
                    .col(ColumnDef::new(UserSshKeys::Fingerprint).string().not_null())
                    .col(
                        ColumnDef::new(UserSshKeys::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSshKeys::ExpiresAt)
                            .timestamp()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserSshKeys::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserSshKeys {
    Table,
    Id,
    UserId,
    Name,
    PublicKey,
    Fingerprint,
    CreatedAt,
    ExpiresAt,
}
