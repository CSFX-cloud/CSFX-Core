use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = "ALTER TABLE workloads ADD COLUMN IF NOT EXISTS replaces_workload_id UUID; \
    ALTER TABLE workloads ADD COLUMN IF NOT EXISTS retire_after TIMESTAMP; \
    ALTER TABLE workloads ADD COLUMN IF NOT EXISTS rollout_error TEXT;";

const DOWN: &str = "DROP INDEX IF EXISTS workloads_replaces_workload_id_key; \
    ALTER TABLE workloads DROP COLUMN IF EXISTS rollout_error; \
    ALTER TABLE workloads DROP COLUMN IF EXISTS retire_after; \
    ALTER TABLE workloads DROP COLUMN IF EXISTS replaces_workload_id;";

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
