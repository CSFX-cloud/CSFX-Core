use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create organizations table
        manager
            .create_table(
                Table::create()
                    .table(Organization::Table)
                    .if_not_exists()
                    .col(uuid(Organization::Id).primary_key())
                    .col(string(Organization::Name))
                    .col(string_null(Organization::Description))
                    .col(timestamp(Organization::CreatedAt))
                    .col(timestamp(Organization::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        // Create roles table
        manager
            .create_table(
                Table::create()
                    .table(Role::Table)
                    .if_not_exists()
                    .col(uuid(Role::Id).primary_key())
                    .col(string(Role::Name))
                    .col(string_null(Role::Description))
                    .col(uuid(Role::OrganizationId))
                    .col(boolean(Role::IsSystemRole).default(false))
                    .col(timestamp(Role::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_organization")
                            .from(Role::Table, Role::OrganizationId)
                            .to(Organization::Table, Organization::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create permissions table
        manager
            .create_table(
                Table::create()
                    .table(Permission::Table)
                    .if_not_exists()
                    .col(uuid(Permission::Id).primary_key())
                    .col(string(Permission::Name))
                    .col(string(Permission::Resource))
                    .col(string(Permission::Action))
                    .col(string_null(Permission::Description))
                    .to_owned(),
            )
            .await?;

        // Create role_permissions junction table
        manager
            .create_table(
                Table::create()
                    .table(RolePermission::Table)
                    .if_not_exists()
                    .col(uuid(RolePermission::RoleId))
                    .col(uuid(RolePermission::PermissionId))
                    .primary_key(
                        Index::create()
                            .col(RolePermission::RoleId)
                            .col(RolePermission::PermissionId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_permission_role")
                            .from(RolePermission::Table, RolePermission::RoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_permission_permission")
                            .from(RolePermission::Table, RolePermission::PermissionId)
                            .to(Permission::Table, Permission::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create user_organizations junction table with role
        manager
            .create_table(
                Table::create()
                    .table(UserOrganization::Table)
                    .if_not_exists()
                    .col(uuid(UserOrganization::Id).primary_key())
                    .col(uuid(UserOrganization::UserId))
                    .col(uuid(UserOrganization::OrganizationId))
                    .col(uuid(UserOrganization::RoleId))
                    .col(timestamp(UserOrganization::JoinedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_organization_user")
                            .from(UserOrganization::Table, UserOrganization::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_organization_organization")
                            .from(UserOrganization::Table, UserOrganization::OrganizationId)
                            .to(Organization::Table, Organization::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_organization_role")
                            .from(UserOrganization::Table, UserOrganization::RoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index on user_id + organization_id
        manager
            .create_index(
                Index::create()
                    .name("idx_user_organization_unique")
                    .table(UserOrganization::Table)
                    .col(UserOrganization::UserId)
                    .col(UserOrganization::OrganizationId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserOrganization::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RolePermission::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Permission::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Role::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Organization::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Organization {
    Table,
    Id,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Role {
    Table,
    Id,
    Name,
    Description,
    OrganizationId,
    IsSystemRole,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Permission {
    Table,
    Id,
    Name,
    Resource,
    Action,
    Description,
}

#[derive(DeriveIden)]
enum RolePermission {
    Table,
    RoleId,
    PermissionId,
}

#[derive(DeriveIden)]
enum UserOrganization {
    Table,
    Id,
    UserId,
    OrganizationId,
    RoleId,
    JoinedAt,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}
