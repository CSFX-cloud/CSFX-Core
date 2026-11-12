use anyhow::Result;
use entity::bootstrap_tokens;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, Set, Statement,
};
use uuid::Uuid;

const CONSUME_SQL: &str = "UPDATE bootstrap_tokens \
    SET use_count = use_count + 1 \
    WHERE token_hash = $1 AND use_count < max_uses AND NOT revoked AND expires_at > $2 \
    RETURNING id, role, use_count, max_uses";

pub struct ConsumedToken {
    pub id: Uuid,
    pub role: String,
    pub use_count: i32,
    pub max_uses: i32,
}

pub async fn create(
    db: &DatabaseConnection,
    token_hash: String,
    description: Option<String>,
    created_by: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    max_uses: i32,
    role: String,
) -> Result<bootstrap_tokens::Model> {
    let model = bootstrap_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        token_hash: Set(token_hash),
        description: Set(description),
        created_by: Set(created_by),
        created_at: Set(chrono::Utc::now().naive_utc()),
        expires_at: Set(expires_at.naive_utc()),
        max_uses: Set(max_uses),
        use_count: Set(0),
        revoked: Set(false),
        revoked_at: Set(None),
        role: Set(role),
    };
    Ok(model.insert(db).await?)
}

pub async fn get_by_hash(
    db: &DatabaseConnection,
    token_hash: &str,
) -> Result<Option<bootstrap_tokens::Model>> {
    Ok(bootstrap_tokens::Entity::find()
        .filter(bootstrap_tokens::Column::TokenHash.eq(token_hash))
        .one(db)
        .await?)
}

pub async fn consume(db: &DatabaseConnection, token_hash: &str) -> Result<Option<ConsumedToken>> {
    let statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        CONSUME_SQL,
        [token_hash.into(), chrono::Utc::now().naive_utc().into()],
    );
    let Some(row) = db.query_one(statement).await? else {
        return Ok(None);
    };
    Ok(Some(ConsumedToken {
        id: row.try_get("", "id")?,
        role: row.try_get("", "role")?,
        use_count: row.try_get("", "use_count")?,
        max_uses: row.try_get("", "max_uses")?,
    }))
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

pub async fn get_all_active(db: &DatabaseConnection) -> Result<Vec<bootstrap_tokens::Model>> {
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
