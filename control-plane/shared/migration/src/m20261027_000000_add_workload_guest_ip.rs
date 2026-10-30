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
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("guest_ip")).string())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS workloads_resource_group_guest_ip_key \
                 ON workloads (resource_group_id, guest_ip) WHERE guest_ip IS NOT NULL",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS workloads_resource_group_guest_ip_key")
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("workloads"))
                    .drop_column(Alias::new("guest_ip"))
                    .to_owned(),
            )
            .await
    }
}
