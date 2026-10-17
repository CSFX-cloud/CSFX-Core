use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE agents ADD COLUMN IF NOT EXISTS machine_id TEXT")
            .await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_agents_machine_id ON agents (machine_id) WHERE machine_id IS NOT NULL",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP INDEX IF EXISTS idx_agents_machine_id")
            .await?;
        db.execute_unprepared("ALTER TABLE agents DROP COLUMN IF EXISTS machine_id")
            .await?;
        Ok(())
    }
}
