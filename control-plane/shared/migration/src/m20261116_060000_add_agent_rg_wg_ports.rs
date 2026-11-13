use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS agent_rg_wg_ports (
    agent_id UUID NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    resource_group_id UUID NOT NULL REFERENCES resource_groups (id) ON DELETE CASCADE,
    port INTEGER NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (agent_id, resource_group_id)
);
CREATE INDEX IF NOT EXISTS idx_agent_rg_wg_ports_resource_group_id
    ON agent_rg_wg_ports (resource_group_id);
"#;

const DOWN: &str = "DROP TABLE IF EXISTS agent_rg_wg_ports;";

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
