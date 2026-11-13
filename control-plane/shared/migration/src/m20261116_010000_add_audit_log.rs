use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_AUDIT_LOG: &str = "
CREATE TABLE IF NOT EXISTS audit_log (
    id UUID PRIMARY KEY,
    service TEXT NOT NULL,
    level TEXT NOT NULL,
    classification TEXT NOT NULL,
    message TEXT NOT NULL,
    agent_id UUID,
    workload_id UUID,
    organization_id UUID,
    created_at TIMESTAMPTZ NOT NULL,
    kind TEXT NOT NULL DEFAULT 'user',
    actor_user_id UUID,
    action TEXT,
    target_type TEXT,
    target_id TEXT,
    fields JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurrences INTEGER NOT NULL DEFAULT 1,
    last_seen_at TIMESTAMPTZ NOT NULL
);
CREATE OR REPLACE FUNCTION audit_log_immutable() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit_log rows are append-only';
END
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS audit_log_no_update ON audit_log;
CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_immutable()";

const COPY_EXISTING: &str = "
INSERT INTO audit_log (id, service, level, classification, message, agent_id, workload_id,
    organization_id, created_at, kind, actor_user_id, action, target_type, target_id, fields,
    occurrences, last_seen_at)
SELECT id, service, level, classification, message, agent_id, workload_id,
    organization_id, created_at, kind, actor_user_id, action, target_type, target_id, fields,
    occurrences, last_seen_at
FROM logs WHERE kind = 'user'
ON CONFLICT (id) DO NOTHING";

const CREATE_INDEXES: &str =
    "CREATE INDEX IF NOT EXISTS idx_audit_log_last_seen_at_id ON audit_log (last_seen_at, id)";

const DOWN: &str = "
DROP TABLE IF EXISTS audit_log;
DROP FUNCTION IF EXISTS audit_log_immutable()";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(CREATE_AUDIT_LOG).await?;
        db.execute_unprepared(COPY_EXISTING).await?;
        db.execute_unprepared(CREATE_INDEXES).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(DOWN).await?;
        Ok(())
    }
}
