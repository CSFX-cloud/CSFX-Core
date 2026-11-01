use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = "
CREATE TABLE IF NOT EXISTS site_usage_hourly (
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    hour TIMESTAMP NOT NULL,
    requests BIGINT NOT NULL DEFAULT 0,
    bytes_out BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (site_id, hour)
)";

const DOWN: &str = "DROP TABLE IF EXISTS site_usage_hourly";

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
