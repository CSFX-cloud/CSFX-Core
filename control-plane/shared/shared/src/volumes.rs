use chrono::Utc;
use entity::entities::volumes;
use sea_orm::sea_query::{Expr, IntoCondition};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    TransactionTrait,
};
use uuid::Uuid;

pub const STATUS_AVAILABLE: &str = "available";
pub const STATUS_IN_USE: &str = "in_use";

pub async fn claim(
    db: &DatabaseConnection,
    volume_ids: &[Uuid],
    agent_id: Uuid,
    workload_id: Option<Uuid>,
) -> Result<bool, DbErr> {
    let mut ordered = volume_ids.to_vec();
    ordered.sort_unstable();
    ordered.dedup();

    let transaction = db.begin().await?;
    for volume_id in ordered {
        if !claim_one(&transaction, volume_id, agent_id, workload_id).await? {
            transaction.rollback().await?;
            return Ok(false);
        }
    }
    transaction.commit().await?;
    Ok(true)
}

pub async fn release(db: &impl ConnectionTrait, volume_id: Uuid) -> Result<bool, DbErr> {
    let released = release_where(db, volumes::Column::Id.eq(volume_id)).await?;
    Ok(released == 1)
}

pub async fn release_for_workload(
    db: &impl ConnectionTrait,
    workload_id: Uuid,
) -> Result<u64, DbErr> {
    release_where(db, volumes::Column::AttachedToWorkload.eq(workload_id)).await
}

async fn claim_one(
    db: &impl ConnectionTrait,
    volume_id: Uuid,
    agent_id: Uuid,
    workload_id: Option<Uuid>,
) -> Result<bool, DbErr> {
    let claimed = volumes::Entity::update_many()
        .col_expr(volumes::Column::AttachedToAgent, Expr::value(agent_id))
        .col_expr(
            volumes::Column::AttachedToWorkload,
            Expr::value(workload_id),
        )
        .col_expr(volumes::Column::Status, Expr::value(STATUS_IN_USE))
        .col_expr(
            volumes::Column::UpdatedAt,
            Expr::value(Utc::now().naive_utc()),
        )
        .filter(volumes::Column::Id.eq(volume_id))
        .filter(volumes::Column::Status.is_in([STATUS_AVAILABLE, STATUS_IN_USE]))
        .filter(claimable_by(agent_id, workload_id))
        .exec(db)
        .await?;
    Ok(claimed.rows_affected == 1)
}

async fn release_where(
    db: &impl ConnectionTrait,
    filter: impl IntoCondition,
) -> Result<u64, DbErr> {
    let released = volumes::Entity::update_many()
        .col_expr(
            volumes::Column::AttachedToAgent,
            Expr::value(Option::<Uuid>::None),
        )
        .col_expr(
            volumes::Column::AttachedToWorkload,
            Expr::value(Option::<Uuid>::None),
        )
        .col_expr(
            volumes::Column::MappedDevice,
            Expr::value(Option::<String>::None),
        )
        .col_expr(volumes::Column::Status, Expr::value(STATUS_AVAILABLE))
        .col_expr(
            volumes::Column::UpdatedAt,
            Expr::value(Utc::now().naive_utc()),
        )
        .filter(filter)
        .exec(db)
        .await?;
    Ok(released.rows_affected)
}

fn claimable_by(agent_id: Uuid, workload_id: Option<Uuid>) -> Condition {
    let mut condition = Condition::any().add(volumes::Column::AttachedToAgent.is_null());
    if let Some(workload_id) = workload_id {
        condition = condition.add(volumes::Column::AttachedToWorkload.eq(workload_id));
    }
    condition.add(
        Condition::all()
            .add(volumes::Column::AttachedToWorkload.is_null())
            .add(volumes::Column::AttachedToAgent.eq(agent_id)),
    )
}
