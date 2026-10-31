mod common;

use chrono::{Duration, SubsecRound, Utc};
use shared::ssh_access::{self, SshAccess};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn persists_and_overwrites_access_window() {
    let db = common::migrated_database().await;

    let opened = SshAccess {
        enabled: true,
        expires_at: Some((Utc::now() + Duration::hours(1)).trunc_subsecs(0)),
    };
    ssh_access::save(&db, &opened).await.unwrap();
    assert_eq!(ssh_access::load(&db).await.unwrap(), opened);
    assert!(ssh_access::load(&db).await.unwrap().is_open());

    let closed = SshAccess {
        enabled: false,
        expires_at: None,
    };
    ssh_access::save(&db, &closed).await.unwrap();
    assert_eq!(ssh_access::load(&db).await.unwrap(), closed);
    assert!(!ssh_access::load(&db).await.unwrap().is_open());
}
