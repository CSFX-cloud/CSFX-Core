use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "ALTER TABLE agents ADD COLUMN IF NOT EXISTS site_id TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE agents ADD COLUMN IF NOT EXISTS zone TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE resource_groups ADD COLUMN IF NOT EXISTS site_id TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE resource_groups ADD COLUMN IF NOT EXISTS zone TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE workloads ADD COLUMN IF NOT EXISTS site_id TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE workloads ADD COLUMN IF NOT EXISTS zone TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE workloads ADD COLUMN IF NOT EXISTS generation BIGINT NOT NULL DEFAULT 1",
    "ALTER TABLE workloads ADD COLUMN IF NOT EXISTS observed_generation BIGINT NOT NULL DEFAULT 0",
    "ALTER TABLE workloads ADD COLUMN IF NOT EXISTS observed_status TEXT",
    "ALTER TABLE workloads ADD COLUMN IF NOT EXISTS topology_spread TEXT NOT NULL DEFAULT 'none'",
    "ALTER TABLE volumes ADD COLUMN IF NOT EXISTS site_id TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE volumes ADD COLUMN IF NOT EXISTS zone TEXT NOT NULL DEFAULT 'default'",
    "ALTER TABLE volumes ADD COLUMN IF NOT EXISTS generation BIGINT NOT NULL DEFAULT 1",
    "ALTER TABLE volumes ADD COLUMN IF NOT EXISTS observed_generation BIGINT NOT NULL DEFAULT 0",
];

const DOWN: &[&str] = &[
    "ALTER TABLE volumes DROP COLUMN IF EXISTS observed_generation, DROP COLUMN IF EXISTS generation, DROP COLUMN IF EXISTS zone, DROP COLUMN IF EXISTS site_id",
    "ALTER TABLE workloads DROP COLUMN IF EXISTS topology_spread, DROP COLUMN IF EXISTS observed_status, DROP COLUMN IF EXISTS observed_generation, DROP COLUMN IF EXISTS generation, DROP COLUMN IF EXISTS zone, DROP COLUMN IF EXISTS site_id",
    "ALTER TABLE resource_groups DROP COLUMN IF EXISTS zone, DROP COLUMN IF EXISTS site_id",
    "ALTER TABLE agents DROP COLUMN IF EXISTS zone, DROP COLUMN IF EXISTS site_id",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for statement in UP {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for statement in DOWN {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }
}
