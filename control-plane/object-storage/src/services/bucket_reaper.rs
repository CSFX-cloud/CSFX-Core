use anyhow::Result;
use chrono::Utc;
use entity::entities::buckets;
use sea_orm::DatabaseConnection;
use shared::leader::LeaderLease;
use tokio::time::{sleep, Duration};

use crate::{db::buckets as db, garage::GarageClient, log_error, log_info, log_warn};

const REAP_INTERVAL: Duration = Duration::from_secs(60);
const PENDING_GRACE_MINUTES: i64 = 10;

pub async fn run(db_conn: DatabaseConnection, garage: GarageClient, leader: LeaderLease) {
    loop {
        sleep(REAP_INTERVAL).await;
        if !leader.is_leader() {
            continue;
        }
        if let Err(e) = reap_once(&db_conn, &garage).await {
            log_error!(
                "services::bucket_reaper",
                &format!("pending bucket reap failed err={}", e)
            );
        }
    }
}

pub async fn reap_once(db_conn: &DatabaseConnection, garage: &GarageClient) -> Result<()> {
    let cutoff = (Utc::now() - chrono::Duration::minutes(PENDING_GRACE_MINUTES)).naive_utc();
    for bucket in db::list_pending_before(db_conn, cutoff).await? {
        if let Err(e) = reap(db_conn, garage, &bucket).await {
            log_warn!(
                "services::bucket_reaper",
                &format!("pending bucket not reaped id={} err={}", bucket.id, e)
            );
        }
    }
    Ok(())
}

async fn reap(
    db_conn: &DatabaseConnection,
    garage: &GarageClient,
    bucket: &buckets::Model,
) -> Result<()> {
    if let Some(key_id) = &bucket.master_key_id {
        garage.delete_key(key_id).await?;
    }
    let garage_bucket_id = match &bucket.garage_bucket_id {
        Some(id) => Some(id.clone()),
        None => garage.find_bucket_id(&bucket.global_alias).await?,
    };
    if let Some(id) = garage_bucket_id {
        garage.delete_bucket(&id).await?;
    }
    db::delete(db_conn, bucket.id).await?;
    log_info!(
        "services::bucket_reaper",
        &format!("pending bucket reaped id={}", bucket.id)
    );
    Ok(())
}
