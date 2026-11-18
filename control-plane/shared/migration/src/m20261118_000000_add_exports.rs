use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_EXPORTS: &str = "
CREATE TABLE IF NOT EXISTS exports (
    id UUID PRIMARY KEY,
    scope_kind TEXT NOT NULL,
    scope_id UUID,
    status TEXT NOT NULL,
    manifest JSONB,
    error TEXT,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_exports_created_at ON exports (created_at DESC)";

const DOWN: &str = "DROP TABLE IF EXISTS exports";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(CREATE_EXPORTS)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(DOWN).await?;
        Ok(())
    }
}
