use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create registry_tokens table
        manager
            .create_table(
                Table::create()
                    .table(RegistryTokens::Table)
                    .if_not_exists()
                    .col(pk_uuid(RegistryTokens::Id))
                    .col(string_uniq(RegistryTokens::Token))
                    .col(string_null(RegistryTokens::Description))
                    .col(string(RegistryTokens::CreatedBy))
                    .col(date_time(RegistryTokens::CreatedAt))
                    .col(date_time(RegistryTokens::ExpiresAt))
                    .col(date_time_null(RegistryTokens::UsedAt))
                    .col(uuid_null(RegistryTokens::UsedByAgentId))
                    .col(boolean(RegistryTokens::IsUsed))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_registry_tokens_agent_id")
                            .from(RegistryTokens::Table, RegistryTokens::UsedByAgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create agent_api_keys table
        manager
            .create_table(
                Table::create()
                    .table(AgentApiKeys::Table)
                    .if_not_exists()
                    .col(pk_uuid(AgentApiKeys::Id))
                    .col(uuid(AgentApiKeys::AgentId))
                    .col(string_uniq(AgentApiKeys::ApiKey))
                    .col(date_time(AgentApiKeys::CreatedAt))
                    .col(date_time_null(AgentApiKeys::LastUsedAt))
                    .col(boolean(AgentApiKeys::IsActive))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_agent_api_keys_agent_id")
                            .from(AgentApiKeys::Table, AgentApiKeys::AgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for token lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_registry_tokens_expires")
                    .table(RegistryTokens::Table)
                    .col(RegistryTokens::ExpiresAt)
                    .col(RegistryTokens::IsUsed)
                    .to_owned(),
            )
            .await?;

        // Create index for agent API key lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_agent_api_keys_agent_active")
                    .table(AgentApiKeys::Table)
                    .col(AgentApiKeys::AgentId)
                    .col(AgentApiKeys::IsActive)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentApiKeys::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(RegistryTokens::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum RegistryTokens {
    Table,
    Id,
    Token,
    Description,
    CreatedBy,
    CreatedAt,
    ExpiresAt,
    UsedAt,
    UsedByAgentId,
    IsUsed,
}

#[derive(DeriveIden)]
enum AgentApiKeys {
    Table,
    Id,
    AgentId,
    ApiKey,
    CreatedAt,
    LastUsedAt,
    IsActive,
}

#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
}
