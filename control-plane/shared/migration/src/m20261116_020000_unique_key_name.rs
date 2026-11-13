use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const DEDUPLICATE_AND_INDEX: &str = r#"
DELETE FROM "key" newer
USING "key" older
WHERE newer.name = older.name AND newer.ctid > older.ctid;
CREATE UNIQUE INDEX IF NOT EXISTS idx_key_name_unique ON "key" (name)"#;

const DROP_INDEX: &str = "DROP INDEX IF EXISTS idx_key_name_unique";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DEDUPLICATE_AND_INDEX)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DROP_INDEX)
            .await?;
        Ok(())
    }
}
