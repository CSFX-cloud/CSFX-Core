use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_VAULTS: &str = "
CREATE TABLE IF NOT EXISTS vaults (
    id UUID PRIMARY KEY,
    resource_group_id UUID NOT NULL REFERENCES resource_groups (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sealed BOOLEAN NOT NULL DEFAULT FALSE,
    soft_delete_days INTEGER NOT NULL DEFAULT 30,
    created_at TIMESTAMP NOT NULL,
    UNIQUE (resource_group_id, name)
)";

const CREATE_VAULT_GRANTS: &str = "
CREATE TABLE IF NOT EXISTS vault_grants (
    id UUID PRIMARY KEY,
    vault_id UUID NOT NULL REFERENCES vaults (id) ON DELETE CASCADE,
    principal TEXT NOT NULL,
    capability TEXT NOT NULL CHECK (capability IN ('read_secrets', 'sign_with_key', 'reveal_secrets', 'manage')),
    scope TEXT NOT NULL DEFAULT '*',
    requires_approval BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL
)";

const CREATE_VAULT_REVEAL_APPROVALS: &str = "
CREATE TABLE IF NOT EXISTS vault_reveal_approvals (
    id UUID PRIMARY KEY,
    vault_id UUID NOT NULL REFERENCES vaults (id) ON DELETE CASCADE,
    secret_name TEXT NOT NULL,
    approved_for UUID NOT NULL,
    approved_by UUID NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL
)";

const CREATE_VAULT_AUDIT: &str = "
CREATE TABLE IF NOT EXISTS vault_audit (
    id UUID PRIMARY KEY,
    vault_id UUID NOT NULL,
    occurred_at TIMESTAMP NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    source TEXT NOT NULL,
    result TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vault_audit_vault_time ON vault_audit (vault_id, occurred_at DESC);
CREATE OR REPLACE FUNCTION vault_audit_immutable() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'vault_audit is append only';
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS vault_audit_no_update ON vault_audit;
CREATE TRIGGER vault_audit_no_update BEFORE UPDATE OR DELETE ON vault_audit
    FOR EACH ROW EXECUTE FUNCTION vault_audit_immutable()";

const DROP_VAULTS: &str = "
DROP TABLE IF EXISTS vault_audit;
DROP FUNCTION IF EXISTS vault_audit_immutable();
DROP TABLE IF EXISTS vault_reveal_approvals;
DROP TABLE IF EXISTS vault_grants;
DROP TABLE IF EXISTS vaults";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        for statement in [
            CREATE_VAULTS,
            CREATE_VAULT_GRANTS,
            CREATE_VAULT_REVEAL_APPROVALS,
            CREATE_VAULT_AUDIT,
        ] {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DROP_VAULTS)
            .await?;
        Ok(())
    }
}
