use sea_orm::{DbConn, DbErr};

const GATEWAY_DEFAULT_MAX_CONNECTIONS: u32 = 20;

pub async fn establish_connection() -> Result<DbConn, DbErr> {
    shared::db::establish_connection_with(GATEWAY_DEFAULT_MAX_CONNECTIONS).await
}
