use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
DELETE FROM resource_group_vpn_peers;
ALTER TABLE resource_group_vpn_peers
    ADD COLUMN user_id UUID NOT NULL REFERENCES "user" (id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_resource_group_vpn_peers_user_id
    ON resource_group_vpn_peers (user_id);
"#;

const DOWN: &str = r#"
DROP INDEX IF EXISTS idx_resource_group_vpn_peers_user_id;
ALTER TABLE resource_group_vpn_peers DROP COLUMN IF EXISTS user_id;
"#;

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
