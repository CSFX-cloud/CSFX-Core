use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const DROP_CONSTRAINT: &str =
    "ALTER TABLE site_domains DROP CONSTRAINT IF EXISTS site_domains_kind_check";

const ADD_WITH_INTERNAL: &str = "
ALTER TABLE site_domains ADD CONSTRAINT site_domains_kind_check
CHECK (kind IN ('production', 'branch', 'auto', 'internal'))";

const DELETE_INTERNAL: &str = "DELETE FROM site_domains WHERE kind = 'internal'";

const ADD_WITHOUT_INTERNAL: &str = "
ALTER TABLE site_domains ADD CONSTRAINT site_domains_kind_check
CHECK (kind IN ('production', 'branch', 'auto'))";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        connection.execute_unprepared(DROP_CONSTRAINT).await?;
        connection.execute_unprepared(ADD_WITH_INTERNAL).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        connection.execute_unprepared(DELETE_INTERNAL).await?;
        connection.execute_unprepared(DROP_CONSTRAINT).await?;
        connection.execute_unprepared(ADD_WITHOUT_INTERNAL).await?;
        Ok(())
    }
}
