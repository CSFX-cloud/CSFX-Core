use migration::{
    build_concurrent_indexes, run_pending_migrations, ConcurrentIndex, Migrator, MigratorTrait,
};
use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
use uuid::Uuid;

fn swap_database(url: &str, name: &str) -> String {
    let (base, _) = url.rsplit_once('/').expect("url must contain a database");
    format!("{base}/{name}")
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn concurrent_migrators_both_succeed() {
    let admin_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let name = format!("migrate_{}", Uuid::new_v4().simple());
    let admin = Database::connect(&admin_url).await.expect("admin connect");
    admin
        .execute_unprepared(&format!("CREATE DATABASE {name}"))
        .await
        .expect("create database");

    let url = swap_database(&admin_url, &name);
    let (first, second) = tokio::join!(run_pending_migrations(&url), run_pending_migrations(&url));
    let pending = {
        let db = Database::connect(&url).await.expect("connect");
        Migrator::get_pending_migrations(&db).await.map(|p| p.len())
    };
    admin
        .execute_unprepared(&format!("DROP DATABASE {name} WITH (FORCE)"))
        .await
        .expect("drop database");

    first.expect("first migrator");
    second.expect("second migrator");
    assert_eq!(pending.expect("pending query"), 0);
}

async fn index_is_valid(db: &DatabaseConnection, name: &str) -> Option<bool> {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!(
                "SELECT i.indisvalid AS valid FROM pg_index i \
                 JOIN pg_class c ON c.oid = i.indexrelid WHERE c.relname = '{name}'"
            ),
        ))
        .await
        .expect("index query")?;
    Some(row.try_get::<bool>("", "valid").expect("valid column"))
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn concurrent_indexes_are_idempotent_and_rebuild_invalid_ones() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = Database::connect(&url).await.expect("connect");
    let table = format!("concurrent_index_{}", Uuid::new_v4().simple());
    let name: &'static str = Box::leak(format!("{table}_idx").into_boxed_str());
    let definition: &'static str = Box::leak(format!("ON {table} (value)").into_boxed_str());
    let indexes = [ConcurrentIndex { name, definition }];
    db.execute_unprepared(&format!("CREATE TABLE {table} (value INT)"))
        .await
        .expect("create table");

    build_concurrent_indexes(&db, &indexes)
        .await
        .expect("first build");
    build_concurrent_indexes(&db, &indexes)
        .await
        .expect("second build");
    db.execute_unprepared(&format!(
        "UPDATE pg_index SET indisvalid = false WHERE indexrelid = '{name}'::regclass"
    ))
    .await
    .expect("invalidate index");
    build_concurrent_indexes(&db, &indexes)
        .await
        .expect("rebuild");
    let valid = index_is_valid(&db, name).await;
    db.execute_unprepared(&format!("DROP TABLE {table}"))
        .await
        .expect("drop table");

    assert_eq!(valid, Some(true));
}
