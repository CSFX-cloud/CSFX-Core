use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("failover_events"))
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("severity"))
                            .string()
                            .not_null()
                            .default("info"),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("status"))
                            .string()
                            .not_null()
                            .default("open"),
                    )
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("resolved_at")).date_time())
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("message")).string())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_failover_events_status")
                    .table(Alias::new("failover_events"))
                    .col(Alias::new("status"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("agents"))
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("maintenance_until")).date_time())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("agents"))
                    .drop_column(Alias::new("maintenance_until"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("failover_events"))
                    .drop_column(Alias::new("severity"))
                    .drop_column(Alias::new("status"))
                    .drop_column(Alias::new("resolved_at"))
                    .drop_column(Alias::new("message"))
                    .to_owned(),
            )
            .await
    }
}
