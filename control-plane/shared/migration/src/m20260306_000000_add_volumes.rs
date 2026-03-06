use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Volumes::Table)
                    .if_not_exists()
                    .col(pk_uuid(Volumes::Id))
                    .col(string(Volumes::Name))
                    .col(integer(Volumes::SizeGb))
                    .col(string(Volumes::Pool))
                    .col(string(Volumes::ImageName))
                    .col(
                        ColumnDef::new(Volumes::Status)
                            .string()
                            .not_null()
                            .default("available"),
                    )
                    .col(uuid_null(Volumes::AttachedToAgent))
                    .col(uuid_null(Volumes::AttachedToWorkload))
                    .col(string_null(Volumes::MappedDevice))
                    .col(date_time(Volumes::CreatedAt))
                    .col(date_time_null(Volumes::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_volumes_agent")
                            .from(Volumes::Table, Volumes::AttachedToAgent)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(VolumeSnapshots::Table)
                    .if_not_exists()
                    .col(pk_uuid(VolumeSnapshots::Id))
                    .col(uuid(VolumeSnapshots::VolumeId))
                    .col(string(VolumeSnapshots::Name))
                    .col(
                        ColumnDef::new(VolumeSnapshots::Status)
                            .string()
                            .not_null()
                            .default("available"),
                    )
                    .col(date_time(VolumeSnapshots::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_snapshots_volume")
                            .from(VolumeSnapshots::Table, VolumeSnapshots::VolumeId)
                            .to(Volumes::Table, Volumes::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_volumes_status")
                    .table(Volumes::Table)
                    .col(Volumes::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_volumes_attached_agent")
                    .table(Volumes::Table)
                    .col(Volumes::AttachedToAgent)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_snapshots_volume")
                    .table(VolumeSnapshots::Table)
                    .col(VolumeSnapshots::VolumeId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VolumeSnapshots::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Volumes::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Volumes {
    Table,
    Id,
    Name,
    SizeGb,
    Pool,
    ImageName,
    Status,
    AttachedToAgent,
    AttachedToWorkload,
    MappedDevice,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum VolumeSnapshots {
    Table,
    Id,
    VolumeId,
    Name,
    Status,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
}
