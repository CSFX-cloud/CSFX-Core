use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE bootstrap_tokens RENAME COLUMN token TO token_hash;
UPDATE bootstrap_tokens
SET token_hash = encode(sha256(convert_to(token_hash, 'UTF8')), 'hex');
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "hashed bootstrap tokens cannot be restored to plaintext".to_string(),
        ))
    }
}
