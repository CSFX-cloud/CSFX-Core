use sea_orm::{Database, DbConn, DbErr};
use std::env;

pub async fn establish_connection() -> Result<DbConn, DbErr> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_conn = Database::connect(&database_url).await?;
    tracing::info!("database connection established");
    Ok(db_conn)
}
