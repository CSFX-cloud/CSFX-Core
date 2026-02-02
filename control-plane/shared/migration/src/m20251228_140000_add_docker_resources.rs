use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create docker_resources table
        manager
            .create_table(
                Table::create()
                    .table(DockerResources::Table)
                    .if_not_exists()
                    .col(pk_uuid(DockerResources::Id))
                    .col(string(DockerResources::Name))
                    .col(string(DockerResources::ResourceType)) // 'docker-container' or 'docker-stack'
                    .col(string_null(DockerResources::Description))
                    .col(uuid(DockerResources::ResourceGroupId))
                    .col(json_null(DockerResources::Configuration))
                    .col(
                        ColumnDef::new(DockerResources::Status)
                            .string()
                            .default("pending")
                            .not_null(),
                    ) // pending, running, stopped, error
                    .col(uuid_null(DockerResources::CreatedBy))
                    .col(date_time(DockerResources::CreatedAt))
                    .col(date_time(DockerResources::UpdatedAt))
                    .col(json_null(DockerResources::Tags))
                    .col(string_null(DockerResources::ContainerId)) // Docker container ID
                    .col(string_null(DockerResources::StackName)) // Docker stack name
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_docker_resources_resource_group_id")
                            .from(DockerResources::Table, DockerResources::ResourceGroupId)
                            .to(ResourceGroups::Table, ResourceGroups::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_docker_resources_created_by")
                            .from(DockerResources::Table, DockerResources::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for resource group lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_docker_resources_resource_group")
                    .table(DockerResources::Table)
                    .col(DockerResources::ResourceGroupId)
                    .to_owned(),
            )
            .await?;

        // Create index for resource type
        manager
            .create_index(
                Index::create()
                    .name("idx_docker_resources_type")
                    .table(DockerResources::Table)
                    .col(DockerResources::ResourceType)
                    .to_owned(),
            )
            .await?;

        // Create index for container_id (for quick Docker lookup)
        manager
            .create_index(
                Index::create()
                    .name("idx_docker_resources_container_id")
                    .table(DockerResources::Table)
                    .col(DockerResources::ContainerId)
                    .to_owned(),
            )
            .await?;

        // Create marketplace_templates table
        manager
            .create_table(
                Table::create()
                    .table(MarketplaceTemplates::Table)
                    .if_not_exists()
                    .col(pk_uuid(MarketplaceTemplates::Id))
                    .col(string(MarketplaceTemplates::TemplateId))
                    .col(string(MarketplaceTemplates::Name))
                    .col(string(MarketplaceTemplates::Description))
                    .col(string(MarketplaceTemplates::Icon))
                    .col(string(MarketplaceTemplates::Category))
                    .col(string(MarketplaceTemplates::ResourceType))
                    .col(json(MarketplaceTemplates::Configuration))
                    .col(boolean(MarketplaceTemplates::Popular).default(false))
                    .col(integer(MarketplaceTemplates::InstallCount).default(0))
                    .col(date_time(MarketplaceTemplates::CreatedAt))
                    .col(date_time(MarketplaceTemplates::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        // Create unique index for template_id
        manager
            .create_index(
                Index::create()
                    .name("idx_marketplace_templates_template_id_unique")
                    .table(MarketplaceTemplates::Table)
                    .col(MarketplaceTemplates::TemplateId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MarketplaceTemplates::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(DockerResources::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum DockerResources {
    Table,
    Id,
    Name,
    ResourceType,
    Description,
    ResourceGroupId,
    Configuration,
    Status,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
    Tags,
    ContainerId,
    StackName,
}

#[derive(DeriveIden)]
enum MarketplaceTemplates {
    Table,
    Id,
    TemplateId,
    Name,
    Description,
    Icon,
    Category,
    ResourceType,
    Configuration,
    Popular,
    InstallCount,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ResourceGroups {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}
