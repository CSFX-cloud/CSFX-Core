use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_INDEXES: [&str; 4] = [
    "CREATE INDEX IF NOT EXISTS idx_invalid_jwt_token ON invalid_jwt (token text_pattern_ops)",
    "CREATE INDEX IF NOT EXISTS idx_ingress_routes_gateway_id ON ingress_routes (gateway_id)",
    "CREATE INDEX IF NOT EXISTS idx_sites_gateway_id ON sites (gateway_id)",
    "CREATE INDEX IF NOT EXISTS idx_site_domains_site_id ON site_domains (site_id)",
];

const DROP_INDEXES: [&str; 4] = [
    "DROP INDEX IF EXISTS idx_invalid_jwt_token",
    "DROP INDEX IF EXISTS idx_ingress_routes_gateway_id",
    "DROP INDEX IF EXISTS idx_sites_gateway_id",
    "DROP INDEX IF EXISTS idx_site_domains_site_id",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for statement in CREATE_INDEXES {
            db.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for statement in DROP_INDEXES {
            db.execute_unprepared(statement).await?;
        }
        Ok(())
    }
}
