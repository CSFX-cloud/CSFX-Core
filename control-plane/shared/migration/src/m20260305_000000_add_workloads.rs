use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Workloads::Table)
                    .if_not_exists()
                    .col(pk_uuid(Workloads::Id))
                    .col(string(Workloads::Name))
                    .col(string(Workloads::Image))
                    .col(integer(Workloads::CpuMillicores))
                    .col(big_integer(Workloads::MemoryBytes))
                    .col(big_integer(Workloads::DiskBytes))
                    .col(json_null(Workloads::EnvVars))
                    .col(json_null(Workloads::Ports))
                    .col(
                        ColumnDef::new(Workloads::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(uuid_null(Workloads::AssignedAgentId))
                    .col(string_null(Workloads::ContainerId))
                    .col(uuid_null(Workloads::CreatedBy))
                    .col(date_time(Workloads::CreatedAt))
                    .col(date_time_null(Workloads::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workloads_assigned_agent")
                            .from(Workloads::Table, Workloads::AssignedAgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workloads_status")
                    .table(Workloads::Table)
                    .col(Workloads::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workloads_assigned_agent")
                    .table(Workloads::Table)
                    .col(Workloads::AssignedAgentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Workloads::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Workloads {
    Table,
    Id,
    Name,
    Image,
    CpuMillicores,
    MemoryBytes,
    DiskBytes,
    EnvVars,
    Ports,
    Status,
    AssignedAgentId,
    ContainerId,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
}
