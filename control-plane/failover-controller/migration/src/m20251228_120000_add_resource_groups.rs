use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create resource_groups table
        manager
            .create_table(
                Table::create()
                    .table(ResourceGroups::Table)
                    .if_not_exists()
                    .col(pk_uuid(ResourceGroups::Id))
                    .col(string(ResourceGroups::Name))
                    .col(string_null(ResourceGroups::Description))
                    .col(uuid(ResourceGroups::OrganizationId))
                    .col(uuid_null(ResourceGroups::CreatedBy))
                    .col(date_time(ResourceGroups::CreatedAt))
                    .col(date_time(ResourceGroups::UpdatedAt))
                    .col(json_null(ResourceGroups::Tags))
                    .col(string_null(ResourceGroups::Location))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_resource_groups_organization_id")
                            .from(ResourceGroups::Table, ResourceGroups::OrganizationId)
                            .to(Organization::Table, Organization::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_resource_groups_created_by")
                            .from(ResourceGroups::Table, ResourceGroups::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for organization lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_resource_groups_organization")
                    .table(ResourceGroups::Table)
                    .col(ResourceGroups::OrganizationId)
                    .to_owned(),
            )
            .await?;

        // Create unique index for name within organization
        manager
            .create_index(
                Index::create()
                    .name("idx_resource_groups_org_name_unique")
                    .table(ResourceGroups::Table)
                    .col(ResourceGroups::OrganizationId)
                    .col(ResourceGroups::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ResourceGroups::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ResourceGroups {
    Table,
    Id,
    Name,
    Description,
    OrganizationId,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
    Tags,
    Location,
}

#[derive(DeriveIden)]
enum Organization {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}
