use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = "
ALTER TABLE ingress_gateways ALTER COLUMN workload_id DROP NOT NULL;
ALTER TABLE ingress_gateways ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active';
CREATE TABLE IF NOT EXISTS idempotency_keys (
    scope TEXT NOT NULL,
    key TEXT NOT NULL,
    path TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    response_status INTEGER NOT NULL,
    response_body BYTEA NOT NULL,
    created_at TIMESTAMP NOT NULL,
    PRIMARY KEY (scope, key)
);
CREATE INDEX IF NOT EXISTS idx_idempotency_keys_created_at ON idempotency_keys (created_at)";

const DOWN: &str = "
DROP TABLE IF EXISTS idempotency_keys;
ALTER TABLE ingress_gateways DROP COLUMN IF EXISTS status;
DELETE FROM ingress_gateways WHERE workload_id IS NULL;
ALTER TABLE ingress_gateways ALTER COLUMN workload_id SET NOT NULL";

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
