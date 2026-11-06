use chrono::Utc;
use entity::entities::volumes;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

const STATUS_IN_USE: &str = "in_use";
const STATUS_AVAILABLE: &str = "available";

pub async fn claim(
    db: &DatabaseConnection,
    volume_ids: &[Uuid],
    agent_id: Uuid,
    workload_id: Uuid,
) -> Result<bool, DbErr> {
    let mut ordered = volume_ids.to_vec();
    ordered.sort_unstable();
    ordered.dedup();

    let transaction = db.begin().await?;
    for volume_id in ordered {
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
            .filter(claimable_by(agent_id, workload_id))
            .exec(&transaction)
            .await?;

        if claimed.rows_affected != 1 {
            transaction.rollback().await?;
            return Ok(false);
        }
    }
    transaction.commit().await?;
    Ok(true)
}

pub async fn release_for_workload(
    db: &DatabaseConnection,
    workload_id: Uuid,
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
        .filter(volumes::Column::AttachedToWorkload.eq(workload_id))
        .exec(db)
        .await?;

    Ok(released.rows_affected)
}

fn claimable_by(agent_id: Uuid, workload_id: Uuid) -> Condition {
    Condition::any()
        .add(volumes::Column::AttachedToAgent.is_null())
        .add(volumes::Column::AttachedToWorkload.eq(workload_id))
        .add(
            Condition::all()
                .add(volumes::Column::AttachedToWorkload.is_null())
                .add(volumes::Column::AttachedToAgent.eq(agent_id)),
        )
}
