use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FailoverEvents::Table)
                    .if_not_exists()
                    .col(pk_uuid(FailoverEvents::Id))
                    .col(uuid_null(FailoverEvents::AgentId))
                    .col(string(FailoverEvents::EventType))
                    .col(json_null(FailoverEvents::AffectedWorkloads))
                    .col(big_integer_null(FailoverEvents::DurationMs))
                    .col(date_time(FailoverEvents::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_failover_events_agent")
                            .from(FailoverEvents::Table, FailoverEvents::AgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_failover_events_agent_id")
                    .table(FailoverEvents::Table)
                    .col(FailoverEvents::AgentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_failover_events_created_at")
                    .table(FailoverEvents::Table)
                    .col(FailoverEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FailoverEvents::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum FailoverEvents {
    Table,
    Id,
    AgentId,
    EventType,
    AffectedWorkloads,
    DurationMs,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
}
