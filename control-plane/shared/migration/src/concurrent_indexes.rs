use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement};

pub struct ConcurrentIndex {
    pub name: &'static str,
    pub definition: &'static str,
    pub unique: bool,
}

pub const CONCURRENT_INDEXES: &[ConcurrentIndex] = &[
    ConcurrentIndex {
        name: "idx_logs_last_seen_at_id",
        definition: "ON logs (last_seen_at, id)",
        unique: false,
    },
    ConcurrentIndex {
        name: "workloads_replaces_workload_id_key",
        definition: "ON workloads (replaces_workload_id) WHERE replaces_workload_id IS NOT NULL",
        unique: true,
    },
];

async fn is_invalid(db: &DatabaseConnection, name: &str) -> Result<bool, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT NOT i.indisvalid AS invalid FROM pg_index i \
             JOIN pg_class c ON c.oid = i.indexrelid WHERE c.relname = $1",
            [name.into()],
        ))
        .await?;
    match row {
        Some(row) => row.try_get::<bool>("", "invalid"),
        None => Ok(false),
    }
}

async fn build(db: &DatabaseConnection, index: &ConcurrentIndex) -> Result<(), DbErr> {
    if is_invalid(db, index.name).await? {
        db.execute_unprepared(&format!("DROP INDEX CONCURRENTLY IF EXISTS {}", index.name))
            .await?;
    }
    let kind = if index.unique {
        "UNIQUE INDEX"
    } else {
        "INDEX"
    };
    db.execute_unprepared(&format!(
        "CREATE {kind} CONCURRENTLY IF NOT EXISTS {} {}",
        index.name, index.definition
    ))
    .await?;
    Ok(())
}

pub async fn build_concurrent_indexes(
    db: &DatabaseConnection,
    indexes: &[ConcurrentIndex],
) -> Result<(), DbErr> {
    for index in indexes {
        build(db, index).await?;
    }
    Ok(())
}
