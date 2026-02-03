use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .add_column(string_null(User::Email))
                    .add_column(string_null(User::TwoFactorSecret))
                    .add_column(boolean(User::TwoFactorEnabled).default(false))
                    .add_column(boolean(User::ForcePasswordChange).default(false))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .drop_column(User::Email)
                    .drop_column(User::TwoFactorSecret)
                    .drop_column(User::TwoFactorEnabled)
                    .drop_column(User::ForcePasswordChange)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    Email,
    TwoFactorSecret,
    TwoFactorEnabled,
    ForcePasswordChange,
}
