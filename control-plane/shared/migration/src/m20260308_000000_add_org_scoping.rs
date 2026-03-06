use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Workloads::Table)
                    .add_column(ColumnDef::new(Workloads::OrganizationId).uuid().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Volumes::Table)
                    .add_column(ColumnDef::new(Volumes::OrganizationId).uuid().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Networks::Table)
                    .add_column(ColumnDef::new(Networks::OrganizationId).uuid().null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Workloads::Table)
                    .drop_column(Workloads::OrganizationId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Volumes::Table)
                    .drop_column(Volumes::OrganizationId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Networks::Table)
                    .drop_column(Networks::OrganizationId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Workloads {
    Table,
    OrganizationId,
}

#[derive(DeriveIden)]
enum Volumes {
    Table,
    OrganizationId,
}

#[derive(DeriveIden)]
enum Networks {
    Table,
    OrganizationId,
}
