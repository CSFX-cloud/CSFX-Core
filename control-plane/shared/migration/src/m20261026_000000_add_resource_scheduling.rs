use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("workloads"))
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("cpu_limit_millicores")).integer(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("memory_limit_bytes")).big_integer(),
                    )
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("spread_key")).string())
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("auto_resources"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("cpu_usage_peak_percent")).double(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("memory_usage_peak_bytes")).big_integer(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("agents"))
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("cpu_overcommit_ratio"))
                            .double()
                            .not_null()
                            .default(4.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("memory_overcommit_ratio"))
                            .double()
                            .not_null()
                            .default(1.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("disk_overcommit_ratio"))
                            .double()
                            .not_null()
                            .default(1.0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("workloads"))
                    .drop_column(Alias::new("cpu_limit_millicores"))
                    .drop_column(Alias::new("memory_limit_bytes"))
                    .drop_column(Alias::new("spread_key"))
                    .drop_column(Alias::new("auto_resources"))
                    .drop_column(Alias::new("cpu_usage_peak_percent"))
                    .drop_column(Alias::new("memory_usage_peak_bytes"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("agents"))
                    .drop_column(Alias::new("cpu_overcommit_ratio"))
                    .drop_column(Alias::new("memory_overcommit_ratio"))
                    .drop_column(Alias::new("disk_overcommit_ratio"))
                    .to_owned(),
            )
            .await
    }
}
