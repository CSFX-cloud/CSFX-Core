use migration::{Migrator, MigratorTrait, run_pending_migrations};
use sea_orm_migration::sea_orm::{ConnectionTrait, Database};
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
