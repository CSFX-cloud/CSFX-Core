use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = "
ALTER TABLE sites ADD COLUMN IF NOT EXISTS gateway_id UUID REFERENCES ingress_gateways (id) ON DELETE SET NULL;
CREATE TABLE IF NOT EXISTS site_artifacts (
    deployment_id UUID PRIMARY KEY REFERENCES site_deployments (id) ON DELETE CASCADE,
    content BYTEA NOT NULL,
    size_bytes BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL
)";

const DOWN: &str = "
DROP TABLE IF EXISTS site_artifacts;
ALTER TABLE sites DROP COLUMN IF EXISTS gateway_id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(DOWN).await?;
        Ok(())
    }
}
