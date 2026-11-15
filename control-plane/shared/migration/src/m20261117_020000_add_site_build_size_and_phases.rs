use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE sites
    ADD COLUMN IF NOT EXISTS build_size TEXT NOT NULL DEFAULT 'small'
    CHECK (build_size IN ('small', 'medium', 'large'));
ALTER TABLE site_deployments ADD COLUMN IF NOT EXISTS build_phases JSONB;
"#;

const DOWN: &str = r#"
ALTER TABLE site_deployments DROP COLUMN IF EXISTS build_phases;
ALTER TABLE sites DROP COLUMN IF EXISTS build_size;
"#;

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
