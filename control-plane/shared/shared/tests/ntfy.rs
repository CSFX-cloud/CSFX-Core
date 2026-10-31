mod common;

use shared::ntfy::{self, NtfyConfig, NtfyError};

fn valid_config() -> NtfyConfig {
    NtfyConfig {
        enabled: true,
        topic: "csfx-alerts".to_string(),
        ..NtfyConfig::default()
    }
}

#[test]
fn rejects_invalid_configuration() {
    let cases = [
        NtfyConfig {
            server_url: "ftp://ntfy.sh".to_string(),
            ..valid_config()
        },
        NtfyConfig {
            topic: String::new(),
            ..valid_config()
        },
        NtfyConfig {
            topic: "a/b".to_string(),
            ..valid_config()
        },
        NtfyConfig {
            min_severity: "fatal".to_string(),
            ..valid_config()
        },
    ];
    for case in cases {
        assert!(matches!(case.validate(), Err(NtfyError::Invalid(_))));
    }
    assert!(valid_config().validate().is_ok());
}

#[test]
fn filters_by_minimum_severity() {
    let config = valid_config();
    assert!(!config.wants("info"));
    assert!(config.wants("warning"));
    assert!(config.wants("critical"));
    assert!(!NtfyConfig {
        enabled: false,
        ..config
    }
    .wants("critical"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn persists_and_updates_configuration() {
    let db = common::migrated_database().await;

    let first = valid_config();
    ntfy::save_config(&db, &first).await.unwrap();
    let loaded = ntfy::load_config(&db).await.unwrap();
    assert_eq!(loaded.topic, first.topic);
    assert!(loaded.enabled);

    let second = NtfyConfig {
        topic: "csfx-other".to_string(),
        token: Some("secret".to_string()),
        min_severity: "critical".to_string(),
        ..first
    };
    ntfy::save_config(&db, &second).await.unwrap();
    let loaded = ntfy::load_config(&db).await.unwrap();
    assert_eq!(loaded.topic, "csfx-other");
    assert_eq!(loaded.token.as_deref(), Some("secret"));
    assert_eq!(loaded.min_severity, "critical");
}
