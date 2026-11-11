use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = "
CREATE TABLE IF NOT EXISTS site_registry_keys (
    id SMALLINT PRIMARY KEY CHECK (id = 1),
    private_key_pem TEXT NOT NULL,
    certificate_pem TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL
)";

const DOWN: &str = "DROP TABLE IF EXISTS site_registry_keys";

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
