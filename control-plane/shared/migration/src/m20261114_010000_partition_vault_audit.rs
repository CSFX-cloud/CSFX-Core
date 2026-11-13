use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const RENAME_LEGACY: &str = "
ALTER TABLE vault_audit RENAME TO vault_audit_legacy;
ALTER INDEX idx_vault_audit_vault_time RENAME TO idx_vault_audit_legacy_vault_time;
DROP TRIGGER vault_audit_no_update ON vault_audit_legacy";

const CREATE_PARTITIONED: &str = "
CREATE TABLE vault_audit (
    id UUID NOT NULL,
    vault_id UUID NOT NULL,
    occurred_at TIMESTAMP NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    source TEXT NOT NULL,
    result TEXT NOT NULL,
    PRIMARY KEY (id, occurred_at)
) PARTITION BY RANGE (occurred_at);
CREATE TABLE vault_audit_default PARTITION OF vault_audit DEFAULT;
CREATE INDEX idx_vault_audit_vault_time ON vault_audit (vault_id, occurred_at DESC)";

const CREATE_MONTHS: &str = "
DO $$
DECLARE
    first_month DATE := date_trunc('month', COALESCE((SELECT MIN(occurred_at) FROM vault_audit_legacy), now()))::date;
    last_month DATE := (date_trunc('month', now()) + INTERVAL '3 months')::date;
    month DATE := first_month;
BEGIN
    WHILE month <= last_month LOOP
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS %I PARTITION OF vault_audit FOR VALUES FROM (%L) TO (%L)',
            'vault_audit_' || to_char(month, 'YYYY_MM'),
            month,
            (month + INTERVAL '1 month')::date
        );
        month := (month + INTERVAL '1 month')::date;
    END LOOP;
END
$$";

const COPY_AND_DROP_LEGACY: &str = "
INSERT INTO vault_audit SELECT id, vault_id, occurred_at, actor, action, target, source, result FROM vault_audit_legacy;
DROP TABLE vault_audit_legacy";

const PROTECT: &str = "
DROP TRIGGER IF EXISTS vault_audit_no_update ON vault_audit;
CREATE TRIGGER vault_audit_no_update BEFORE UPDATE OR DELETE ON vault_audit
    FOR EACH ROW EXECUTE FUNCTION vault_audit_immutable()";

const RESTORE_PLAIN: &str = "
CREATE TABLE vault_audit_plain (
    id UUID PRIMARY KEY,
    vault_id UUID NOT NULL,
    occurred_at TIMESTAMP NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    source TEXT NOT NULL,
    result TEXT NOT NULL
);
INSERT INTO vault_audit_plain SELECT id, vault_id, occurred_at, actor, action, target, source, result FROM vault_audit;
DROP TABLE vault_audit;
ALTER TABLE vault_audit_plain RENAME TO vault_audit;
CREATE INDEX idx_vault_audit_vault_time ON vault_audit (vault_id, occurred_at DESC);
CREATE TRIGGER vault_audit_no_update BEFORE UPDATE OR DELETE ON vault_audit
    FOR EACH ROW EXECUTE FUNCTION vault_audit_immutable()";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        for statement in [
            RENAME_LEGACY,
            CREATE_PARTITIONED,
            CREATE_MONTHS,
            COPY_AND_DROP_LEGACY,
            PROTECT,
        ] {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(RESTORE_PLAIN)
            .await?;
        Ok(())
    }
}
