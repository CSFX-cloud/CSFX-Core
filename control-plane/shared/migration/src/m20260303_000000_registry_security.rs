use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RegistryTokens::Table)
                    .add_column(uuid_null(RegistryTokens::AgentId))
                    .add_column(string(RegistryTokens::ExpectedName).default(""))
                    .add_column(string(RegistryTokens::ExpectedHostname).default(""))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(AgentApiKeys::Table)
                    .add_column(string(AgentApiKeys::KeyHash).default(""))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_tokens_agent_id")
                    .table(RegistryTokens::Table)
                    .col(RegistryTokens::AgentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_agent_api_keys_hash")
                    .table(AgentApiKeys::Table)
                    .col(AgentApiKeys::KeyHash)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_agent_api_keys_hash")
                    .table(AgentApiKeys::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_registry_tokens_agent_id")
                    .table(RegistryTokens::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(AgentApiKeys::Table)
                    .drop_column(AgentApiKeys::KeyHash)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(RegistryTokens::Table)
                    .drop_column(RegistryTokens::ExpectedHostname)
                    .drop_column(RegistryTokens::ExpectedName)
                    .drop_column(RegistryTokens::AgentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum RegistryTokens {
    Table,
    AgentId,
    ExpectedName,
    ExpectedHostname,
}

#[derive(DeriveIden)]
enum AgentApiKeys {
    Table,
    KeyHash,
}
