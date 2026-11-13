use sea_orm::{DbConn, DbErr};

pub const USER_DEFAULT_MAX_CONNECTIONS: u32 = 20;
pub const AGENT_DEFAULT_MAX_CONNECTIONS: u32 = 20;
pub const LOG_DEFAULT_MAX_CONNECTIONS: u32 = 2;

pub async fn establish_connection() -> Result<DbConn, DbErr> {
    shared::db::establish_pool("", USER_DEFAULT_MAX_CONNECTIONS).await
}
