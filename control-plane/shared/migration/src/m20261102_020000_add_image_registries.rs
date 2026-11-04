use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_IMAGE_REGISTRIES: &str = "
CREATE TABLE IF NOT EXISTS image_registries (
    id UUID PRIMARY KEY,
    resource_group_id UUID NOT NULL REFERENCES resource_groups (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    UNIQUE (resource_group_id, name)
)";

const CREATE_IMAGE_REGISTRY_TOKENS: &str = "
CREATE TABLE IF NOT EXISTS image_registry_tokens (
    id UUID PRIMARY KEY,
    registry_id UUID NOT NULL REFERENCES image_registries (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL CHECK (scope IN ('pull', 'push')),
    created_at TIMESTAMP NOT NULL,
    last_used_at TIMESTAMP
)";

const DROP_IMAGE_REGISTRIES: &str = "
DROP TABLE IF EXISTS image_registry_tokens;
DROP TABLE IF EXISTS image_registries";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        for statement in [CREATE_IMAGE_REGISTRIES, CREATE_IMAGE_REGISTRY_TOKENS] {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DROP_IMAGE_REGISTRIES)
            .await?;
        Ok(())
    }
}
