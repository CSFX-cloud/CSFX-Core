use chrono::Utc;
use entity::entities::workload_stacks;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter,
};
use shared::env_secrets::{self, EnvSecretError};
use uuid::Uuid;

use crate::services::compose_redact;

fn custom_error(error: EnvSecretError) -> DbErr {
    DbErr::Custom(error.to_string())
}

pub fn display_source(model: &workload_stacks::Model) -> Option<String> {
    let stored = model.compose_source.as_deref()?;
    match env_secrets::open_text(stored, &env_secrets::stack_source(model.id)) {
        Ok(source) => compose_redact::redact(&source),
        Err(error) => {
            tracing::error!(stack = %model.id, error = %error, "stack source could not be read");
            None
        }
    }
}

pub async fn seal_legacy_sources(db: &DatabaseConnection) -> Result<u64, DbErr> {
    let rows = workload_stacks::Entity::find()
        .filter(workload_stacks::Column::ComposeSource.not_like("enc:v1:%"))
        .all(db)
        .await?;
    let mut sealed = 0;
    for row in rows {
        let Some(source) = row
            .compose_source
            .as_deref()
            .filter(|text| !env_secrets::is_sealed_text(text))
        else {
            continue;
        };
        let replacement = env_secrets::seal_text(source, &env_secrets::stack_source(row.id))
            .map_err(custom_error)?;
        let applied = workload_stacks::Entity::update_many()
            .col_expr(
                workload_stacks::Column::ComposeSource,
                Expr::value(replacement),
            )
            .filter(workload_stacks::Column::Id.eq(row.id))
            .filter(workload_stacks::Column::ComposeSource.eq(source))
            .exec(db)
            .await?;
        sealed += applied.rows_affected;
    }
    Ok(sealed)
}

pub async fn create(
    db: &DatabaseConnection,
    resource_group_id: Uuid,
    name: &str,
    compose_source: &str,
) -> Result<workload_stacks::Model, sea_orm::DbErr> {
    let id = Uuid::new_v4();
    let sealed = env_secrets::seal_text(compose_source, &env_secrets::stack_source(id))
        .map_err(custom_error)?;
    let model = workload_stacks::ActiveModel {
        id: Set(id),
        resource_group_id: Set(resource_group_id),
        name: Set(name.to_string()),
        compose_source: Set(Some(sealed)),
        status: Set("pending".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    };

    model.insert(db).await
}

pub async fn update_status(
    db: &DatabaseConnection,
    stack_id: Uuid,
    status: &str,
) -> Result<(), sea_orm::DbErr> {
    let stack = workload_stacks::Entity::find_by_id(stack_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(stack_id.to_string()))?;

    let mut active: workload_stacks::ActiveModel = stack.into();
    active.status = Set(status.to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    active.update(db).await?;
    Ok(())
}

pub async fn update_compose_source(
    db: &DatabaseConnection,
    stack_id: Uuid,
    compose_source: &str,
) -> Result<(), sea_orm::DbErr> {
    let stack = workload_stacks::Entity::find_by_id(stack_id)
        .one(db)
        .await?
        .ok_or(sea_orm::DbErr::RecordNotFound(stack_id.to_string()))?;

    let context = env_secrets::stack_source(stack_id);
    let previous = stack
        .compose_source
        .as_deref()
        .and_then(|stored| env_secrets::open_text(stored, &context).ok());
    let restored = previous
        .and_then(|previous| compose_redact::restore(compose_source, &previous))
        .unwrap_or_else(|| compose_source.to_string());
    let sealed = env_secrets::seal_text(&restored, &context).map_err(custom_error)?;
    let mut active: workload_stacks::ActiveModel = stack.into();
    active.compose_source = Set(Some(sealed));
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    active.update(db).await?;
    Ok(())
}

pub async fn get_by_id(
    db: &DatabaseConnection,
    stack_id: Uuid,
) -> Result<Option<workload_stacks::Model>, sea_orm::DbErr> {
    workload_stacks::Entity::find_by_id(stack_id).one(db).await
}

pub async fn delete(db: &DatabaseConnection, stack_id: Uuid) -> Result<(), sea_orm::DbErr> {
    workload_stacks::Entity::delete_by_id(stack_id)
        .exec(db)
        .await?;
    Ok(())
}
