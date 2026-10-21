use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_GATEWAYS: &str = "
CREATE TABLE IF NOT EXISTS ingress_gateways (
    id UUID PRIMARY KEY,
    resource_group_id UUID NOT NULL REFERENCES resource_groups (id) ON DELETE CASCADE,
    workload_id UUID NOT NULL UNIQUE REFERENCES workloads (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    http_port INTEGER NOT NULL,
    https_port INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    UNIQUE (resource_group_id, name)
)";

const ADD_ROUTE_GATEWAY: &str = "
ALTER TABLE ingress_routes
    ADD COLUMN IF NOT EXISTS gateway_id UUID REFERENCES ingress_gateways (id) ON DELETE CASCADE";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(CREATE_GATEWAYS).await?;
        db.execute_unprepared(ADD_ROUTE_GATEWAY).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE ingress_routes DROP COLUMN IF EXISTS gateway_id")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS ingress_gateways")
            .await?;
        Ok(())
    }
}
