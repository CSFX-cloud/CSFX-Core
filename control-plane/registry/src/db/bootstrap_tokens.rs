use anyhow::Result;
use entity::bootstrap_tokens;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub async fn create(
    db: &DatabaseConnection,
    token: String,
    description: Option<String>,
    created_by: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    max_uses: i32,
) -> Result<bootstrap_tokens::Model> {
    let model = bootstrap_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        token: Set(token),
        description: Set(description),
        created_by: Set(created_by),
        created_at: Set(chrono::Utc::now().naive_utc()),
        expires_at: Set(expires_at.naive_utc()),
        max_uses: Set(max_uses),
        use_count: Set(0),
        revoked: Set(false),
        revoked_at: Set(None),
    };
    Ok(model.insert(db).await?)
}

pub async fn get_by_token(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<bootstrap_tokens::Model>> {
    Ok(bootstrap_tokens::Entity::find()
        .filter(bootstrap_tokens::Column::Token.eq(token))
        .one(db)
        .await?)
}

pub async fn increment_use_count(db: &DatabaseConnection, id: Uuid) -> Result<()> {
    let mut model: bootstrap_tokens::ActiveModel = bootstrap_tokens::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Bootstrap token not found"))?
        .into();

    let current = match &model.use_count {
        sea_orm::ActiveValue::Unchanged(v) | sea_orm::ActiveValue::Set(v) => *v,
        _ => 0,
    };
    model.use_count = Set(current + 1);
    model.update(db).await?;
    Ok(())
}

pub async fn revoke(db: &DatabaseConnection, id: Uuid) -> Result<()> {
    let mut model: bootstrap_tokens::ActiveModel = bootstrap_tokens::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Bootstrap token not found"))?
        .into();

    model.revoked = Set(true);
    model.revoked_at = Set(Some(chrono::Utc::now().naive_utc()));
    model.update(db).await?;
    Ok(())
}

pub async fn get_all_active(
    db: &DatabaseConnection,
) -> Result<Vec<bootstrap_tokens::Model>> {
    Ok(bootstrap_tokens::Entity::find()
        .filter(bootstrap_tokens::Column::Revoked.eq(false))
        .all(db)
        .await?)
}

pub async fn delete_expired(db: &DatabaseConnection) -> Result<u64> {
    let now = chrono::Utc::now().naive_utc();
    let result = bootstrap_tokens::Entity::delete_many()
        .filter(bootstrap_tokens::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}
