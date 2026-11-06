use crate::common::{mock_registry, registry_client};

#[tokio::test]
async fn images_list_every_tag_with_size_and_digest() {
    let mock = mock_registry(&[
        ("rg-1/app", &["v1", "v2"]),
        ("rg-1/db", &["v1"]),
        ("other/x", &["v1"]),
    ])
    .await;
    let images = registry_client(&mock.address)
        .images("rg-1/")
        .await
        .unwrap();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].repository, "rg-1/app");
    assert_eq!(images[0].tags.len(), 2);
    assert_eq!(images[0].tags[0].size_bytes, Some(100));
    assert!(images[0].tags[0].digest.is_some());
}

#[tokio::test]
async fn delete_tag_removes_known_tags_and_ignores_unknown_ones() {
    let mock = mock_registry(&[("rg-1/app", &["v1", "v2"])]).await;
    let client = registry_client(&mock.address);
    client.delete_tag("rg-1/app", "v1").await.unwrap();
    client.delete_tag("rg-1/app", "ghost").await.unwrap();
    client.delete_tag("rg-1/missing", "v1").await.unwrap();
    assert_eq!(
        mock.state.repositories.lock().unwrap()["rg-1/app"],
        vec!["v2"]
    );
}

#[tokio::test]
async fn purge_prefix_and_delete_repository_empty_repositories() {
    let mock = mock_registry(&[
        ("rg-1/app", &["v1"]),
        ("rg-1/db", &["v1", "v2"]),
        ("keep/x", &["v1"]),
    ])
    .await;
    let client = registry_client(&mock.address);
    client.purge_prefix("rg-1/").await.unwrap();
    {
        let repositories = mock.state.repositories.lock().unwrap();
        assert!(repositories["rg-1/app"].is_empty());
        assert!(repositories["rg-1/db"].is_empty());
        assert_eq!(repositories["keep/x"], vec!["v1"]);
    }
    client.delete_repository("keep/x").await.unwrap();
    assert!(client.tags("unknown/repo").await.unwrap().is_empty());
}

#[tokio::test]
async fn unreachable_registries_are_reported() {
    let client = registry_client("127.0.0.1:1");
    assert!(client.repositories("x").await.is_err());
    assert!(client.tags("x").await.is_err());
    assert!(client.delete_tag("x", "v1").await.is_err());
}
