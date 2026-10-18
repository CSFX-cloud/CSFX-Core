use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP_STATEMENTS: [&str; 8] = [
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'system'",
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS actor_user_id UUID",
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS action TEXT",
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS target_type TEXT",
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS target_id TEXT",
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS fields JSONB NOT NULL DEFAULT '{}'::jsonb",
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS occurrences INTEGER NOT NULL DEFAULT 1",
    "ALTER TABLE logs ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now()",
];

const INDEX_STATEMENTS: [&str; 3] = [
    "CREATE INDEX IF NOT EXISTS idx_logs_kind_last_seen ON logs (kind, last_seen_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_logs_agent_last_seen ON logs (agent_id, last_seen_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_logs_actor_last_seen ON logs (actor_user_id, last_seen_at DESC)",
];

const DOWN_COLUMNS: [&str; 8] = [
    "kind",
    "actor_user_id",
    "action",
    "target_type",
    "target_id",
    "fields",
    "occurrences",
    "last_seen_at",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for statement in UP_STATEMENTS.iter().chain(INDEX_STATEMENTS.iter()) {
            db.execute_unprepared(statement).await?;
        }
        db.execute_unprepared("UPDATE logs SET last_seen_at = created_at")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for column in DOWN_COLUMNS {
            db.execute_unprepared(&format!("ALTER TABLE logs DROP COLUMN IF EXISTS {column}"))
                .await?;
        }
        Ok(())
    }
}
