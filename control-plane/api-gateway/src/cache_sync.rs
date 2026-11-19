use crate::{auth::middleware::forget_live_sessions, rbac_service::forget_granted_permissions};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use std::time::Duration;
use tracing::{debug, warn};

const CHANNEL: &str = "csfx_auth_cache";
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

fn clear_local() {
    forget_live_sessions();
    forget_granted_permissions();
}

pub async fn invalidate(db: &DatabaseConnection) {
    clear_local();
    let statement = format!("SELECT pg_notify('{CHANNEL}', '')");
    if let Err(error) = db.execute_unprepared(&statement).await {
        warn!(error = %error, "auth cache invalidation not broadcast");
    }
}

pub fn spawn(db: DatabaseConnection) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = listen(&db).await {
                warn!(error = %error, "auth cache listener failed");
            }
            clear_local();
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    });
}

async fn listen(db: &DatabaseConnection) -> Result<(), sea_orm::sqlx::Error> {
    let mut listener =
        sea_orm::sqlx::postgres::PgListener::connect_with(db.get_postgres_connection_pool())
            .await?;
    listener.listen(CHANNEL).await?;
    debug!(channel = CHANNEL, "auth cache listener started");
    loop {
        listener.recv().await?;
        clear_local();
    }
}
