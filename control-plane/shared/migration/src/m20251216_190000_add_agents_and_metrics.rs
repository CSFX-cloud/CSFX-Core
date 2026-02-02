use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create agents table
        manager
            .create_table(
                Table::create()
                    .table(Agents::Table)
                    .if_not_exists()
                    .col(pk_uuid(Agents::Id))
                    .col(string(Agents::Name))
                    .col(string(Agents::Hostname))
                    .col(string_null(Agents::IpAddress))
                    .col(string(Agents::AgentVersion))
                    .col(string(Agents::OsType))
                    .col(string(Agents::OsVersion))
                    .col(string(Agents::Architecture))
                    .col(string(Agents::Status))
                    .col(date_time_null(Agents::LastHeartbeat))
                    .col(date_time(Agents::RegisteredAt))
                    .col(date_time_null(Agents::UpdatedAt))
                    .col(uuid_null(Agents::OrganizationId))
                    .col(json_null(Agents::Tags))
                    .col(json_null(Agents::Capabilities))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_agents_organization_id")
                            .from(Agents::Table, Agents::OrganizationId)
                            .to(Organization::Table, Organization::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create agent_metrics table
        manager
            .create_table(
                Table::create()
                    .table(AgentMetrics::Table)
                    .if_not_exists()
                    .col(pk_uuid(AgentMetrics::Id))
                    .col(uuid(AgentMetrics::AgentId))
                    .col(date_time(AgentMetrics::Timestamp))
                    // CPU
                    .col(string_null(AgentMetrics::CpuModel))
                    .col(integer_null(AgentMetrics::CpuCores))
                    .col(integer_null(AgentMetrics::CpuThreads))
                    .col(float_null(AgentMetrics::CpuUsagePercent))
                    // Memory
                    .col(big_integer_null(AgentMetrics::MemoryTotalBytes))
                    .col(big_integer_null(AgentMetrics::MemoryUsedBytes))
                    .col(float_null(AgentMetrics::MemoryUsagePercent))
                    // Disk
                    .col(big_integer_null(AgentMetrics::DiskTotalBytes))
                    .col(big_integer_null(AgentMetrics::DiskUsedBytes))
                    .col(float_null(AgentMetrics::DiskUsagePercent))
                    // Network
                    .col(big_integer_null(AgentMetrics::NetworkRxBytes))
                    .col(big_integer_null(AgentMetrics::NetworkTxBytes))
                    // System
                    .col(string_null(AgentMetrics::OsName))
                    .col(string_null(AgentMetrics::OsVersion))
                    .col(string_null(AgentMetrics::KernelVersion))
                    .col(string_null(AgentMetrics::Hostname))
                    .col(big_integer_null(AgentMetrics::UptimeSeconds))
                    // Custom
                    .col(json_null(AgentMetrics::CustomMetrics))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_agent_metrics_agent_id")
                            .from(AgentMetrics::Table, AgentMetrics::AgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for time-series queries
        manager
            .create_index(
                Index::create()
                    .name("idx_agent_metrics_agent_time")
                    .table(AgentMetrics::Table)
                    .col(AgentMetrics::AgentId)
                    .col(AgentMetrics::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentMetrics::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Agents::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
    Name,
    Hostname,
    IpAddress,
    AgentVersion,
    OsType,
    OsVersion,
    Architecture,
    Status,
    LastHeartbeat,
    RegisteredAt,
    UpdatedAt,
    OrganizationId,
    Tags,
    Capabilities,
}

#[derive(DeriveIden)]
enum AgentMetrics {
    Table,
    Id,
    AgentId,
    Timestamp,
    CpuModel,
    CpuCores,
    CpuThreads,
    CpuUsagePercent,
    MemoryTotalBytes,
    MemoryUsedBytes,
    MemoryUsagePercent,
    DiskTotalBytes,
    DiskUsedBytes,
    DiskUsagePercent,
    NetworkRxBytes,
    NetworkTxBytes,
    OsName,
    OsVersion,
    KernelVersion,
    Hostname,
    UptimeSeconds,
    CustomMetrics,
}

#[derive(DeriveIden)]
enum Organization {
    Table,
    Id,
}
