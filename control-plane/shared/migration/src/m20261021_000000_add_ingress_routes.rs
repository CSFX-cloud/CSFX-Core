use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum IngressRoutes {
    Table,
    Id,
    ResourceGroupId,
    WorkloadId,
    Hostname,
    ContainerPort,
    Tls,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IngressRoutes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IngressRoutes::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(IngressRoutes::ResourceGroupId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IngressRoutes::WorkloadId).uuid().not_null())
                    .col(
                        ColumnDef::new(IngressRoutes::Hostname)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(IngressRoutes::ContainerPort)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IngressRoutes::Tls)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(IngressRoutes::CreatedAt)
                            .date_time()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(IngressRoutes::Table, IngressRoutes::ResourceGroupId)
                            .to(Alias::new("resource_groups"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(IngressRoutes::Table, IngressRoutes::WorkloadId)
                            .to(Alias::new("workloads"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IngressRoutes::Table).to_owned())
            .await
    }
}
