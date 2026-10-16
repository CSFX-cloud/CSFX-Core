use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_role_column(manager, "agents", "master").await?;
        add_role_column(manager, "bootstrap_tokens", "worker").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_role_column(manager, "bootstrap_tokens").await?;
        drop_role_column(manager, "agents").await
    }
}

async fn add_role_column(
    manager: &SchemaManager<'_>,
    table: &str,
    default_role: &str,
) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Alias::new(table))
                .add_column_if_not_exists(
                    ColumnDef::new(Alias::new("role"))
                        .string()
                        .not_null()
                        .default(default_role),
                )
                .to_owned(),
        )
        .await
}

async fn drop_role_column(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Alias::new(table))
                .drop_column(Alias::new("role"))
                .to_owned(),
        )
        .await
}
