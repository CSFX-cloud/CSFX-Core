use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("resource_groups"))
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("icon_image")).binary())
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("icon_image_mime")).string(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("resource_groups"))
                    .drop_column(Alias::new("icon_image"))
                    .drop_column(Alias::new("icon_image_mime"))
                    .to_owned(),
            )
            .await
    }
}
