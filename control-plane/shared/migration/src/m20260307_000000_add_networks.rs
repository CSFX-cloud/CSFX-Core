use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Networks::Table)
                    .if_not_exists()
                    .col(pk_uuid(Networks::Id))
                    .col(string(Networks::Name))
                    .col(string(Networks::Cidr))
                    .col(string(Networks::OverlayType))
                    .col(string(Networks::Status))
                    .col(date_time(Networks::CreatedAt))
                    .col(date_time_null(Networks::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NetworkPolicies::Table)
                    .if_not_exists()
                    .col(pk_uuid(NetworkPolicies::Id))
                    .col(uuid(NetworkPolicies::NetworkId))
                    .col(string(NetworkPolicies::Direction))
                    .col(string(NetworkPolicies::Action))
                    .col(string_null(NetworkPolicies::SourceCidr))
                    .col(string_null(NetworkPolicies::DestinationCidr))
                    .col(integer_null(NetworkPolicies::Port))
                    .col(string_null(NetworkPolicies::Protocol))
                    .col(integer(NetworkPolicies::Priority))
                    .col(date_time(NetworkPolicies::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_network_policies_network")
                            .from(NetworkPolicies::Table, NetworkPolicies::NetworkId)
                            .to(Networks::Table, Networks::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NetworkMembers::Table)
                    .if_not_exists()
                    .col(pk_uuid(NetworkMembers::Id))
                    .col(uuid(NetworkMembers::NetworkId))
                    .col(uuid(NetworkMembers::WorkloadId))
                    .col(string(NetworkMembers::AllocatedIp))
                    .col(date_time(NetworkMembers::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_network_members_network")
                            .from(NetworkMembers::Table, NetworkMembers::NetworkId)
                            .to(Networks::Table, Networks::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_network_policies_network_id")
                    .table(NetworkPolicies::Table)
                    .col(NetworkPolicies::NetworkId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_network_members_network_id")
                    .table(NetworkMembers::Table)
                    .col(NetworkMembers::NetworkId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_network_members_workload_id")
                    .table(NetworkMembers::Table)
                    .col(NetworkMembers::WorkloadId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NetworkMembers::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NetworkPolicies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Networks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Networks {
    Table,
    Id,
    Name,
    Cidr,
    OverlayType,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum NetworkPolicies {
    Table,
    Id,
    NetworkId,
    Direction,
    Action,
    SourceCidr,
    DestinationCidr,
    Port,
    Protocol,
    Priority,
    CreatedAt,
}

#[derive(DeriveIden)]
enum NetworkMembers {
    Table,
    Id,
    NetworkId,
    WorkloadId,
    AllocatedIp,
    CreatedAt,
}
