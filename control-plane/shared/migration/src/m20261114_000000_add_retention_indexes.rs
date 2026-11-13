use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS idx_agent_metrics_timestamp ON agent_metrics (timestamp);
CREATE INDEX IF NOT EXISTS idx_invalid_jwt_exp ON invalid_jwt (exp)";

const DROP_INDEXES: &str = "
DROP INDEX IF EXISTS idx_agent_metrics_timestamp;
DROP INDEX IF EXISTS idx_invalid_jwt_exp";

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
