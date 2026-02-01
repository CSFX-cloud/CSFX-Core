use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Subscriptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Subscriptions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Subscriptions::UserId).uuid().not_null())
                    .col(ColumnDef::new(Subscriptions::Name).string().not_null())
                    .col(ColumnDef::new(Subscriptions::Amount).decimal().not_null())
                    .col(
                        ColumnDef::new(Subscriptions::BillingCycle)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::NextBillingDate)
                            .date_time()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Subscriptions::Category).string().not_null())
                    .col(
                        ColumnDef::new(Subscriptions::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_subscription_user")
                            .from(Subscriptions::Table, Subscriptions::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Subscriptions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Subscriptions {
    Table,
    Id,
    UserId,
    Name,
    Amount,
    BillingCycle,
    NextBillingDate,
    Category,
    IsActive,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}
