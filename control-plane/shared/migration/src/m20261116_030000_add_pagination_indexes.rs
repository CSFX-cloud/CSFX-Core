use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS idx_failover_events_status_created_at ON failover_events (status, created_at DESC, id DESC)";

const DROP_INDEXES: &str = "
DROP INDEX IF EXISTS idx_failover_events_status_created_at";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(CREATE_INDEXES)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DROP_INDEXES)
            .await?;
        Ok(())
    }
}
