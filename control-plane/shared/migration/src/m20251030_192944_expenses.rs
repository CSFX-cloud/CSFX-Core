use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Expenses::Table)
                    .if_not_exists()
                    .col(pk_uuid(Expenses::Id))
                    .col(uuid(Expenses::UserId))
                    .col(string(Expenses::Description))
                    .col(decimal(Expenses::Amount))
                    .col(date_time(Expenses::Date))
                    .col(string(Expenses::Category))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_expenses_user_id")
                            .from(Expenses::Table, Expenses::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Expenses::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Expenses {
    Table,
    Id,
    UserId,
    Description,
    Amount,
    Date,
    Category,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}
