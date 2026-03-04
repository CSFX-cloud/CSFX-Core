use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AgentApiKeys::Table)
                    .drop_column(AgentApiKeys::ApiKey)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AgentApiKeys::Table)
                    .add_column(
                        ColumnDef::new(AgentApiKeys::ApiKey)
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AgentApiKeys {
    Table,
    ApiKey,
}
