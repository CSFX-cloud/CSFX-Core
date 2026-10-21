use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_DNS_RECORDS: &str = "
CREATE TABLE IF NOT EXISTS dns_records (
    id UUID PRIMARY KEY,
    resource_group_id UUID NOT NULL REFERENCES resource_groups (id) ON DELETE CASCADE,
    name TEXT NOT NULL UNIQUE,
    record_type TEXT NOT NULL CHECK (record_type IN ('A', 'CNAME')),
    value TEXT NOT NULL,
    ttl INTEGER NOT NULL DEFAULT 30,
    created_at TIMESTAMP NOT NULL
)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(CREATE_DNS_RECORDS)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS dns_records")
            .await?;
        Ok(())
    }
}
