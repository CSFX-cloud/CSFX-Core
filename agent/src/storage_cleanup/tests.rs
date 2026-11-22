use super::*;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("csfx-cleanup-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn age_by(path: &Path, age: Duration) {
    let file = std::fs::File::open(path).unwrap();
    file.set_modified(SystemTime::now() - age).unwrap();
}

fn vm_dir(root: &Path, name: &str, age: Duration) -> PathBuf {
    let path = root.join(name);
    std::fs::create_dir_all(&path).unwrap();
    age_by(&path, age);
    path
}

fn image(dir: &Path, name: &str, age: Duration) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"x").unwrap();
    age_by(&path, age);
    path
}

#[tokio::test]
async fn expired_vm_dirs_without_a_workload_are_removed() {
    let root = temp_dir();
    let stale = vm_dir(&root, "gone", DAY * 2);
    let fresh = vm_dir(&root, "recent", DAY / 2);
    let wanted = vm_dir(&root, "wanted", DAY * 2);
    let desired = HashSet::from(["wanted", "other"]);

    let removed = remove_orphaned_vm_dirs(&root, &desired, SystemTime::now()).await;

    assert_eq!(removed, 1);
    assert!(!stale.exists());
    assert!(fresh.exists());
    assert!(wanted.exists());
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn nothing_is_removed_while_no_workload_is_known() {
    let root = temp_dir();
    let stale = vm_dir(&root, "gone", DAY * 2);

    let removed = remove_orphaned_vm_dirs(&root, &HashSet::new(), SystemTime::now()).await;

    assert_eq!(removed, 0);
    assert!(stale.exists());
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn missing_directories_are_ignored() {
    let missing = temp_dir().join("absent");
    let desired = HashSet::from(["a"]);

    assert_eq!(
        remove_orphaned_vm_dirs(&missing, &desired, SystemTime::now()).await,
        0
    );
    assert_eq!(
        remove_superseded_rootfs(&missing, SystemTime::now()).await,
        0
    );
}

#[tokio::test]
async fn only_old_rootfs_images_with_a_newer_sibling_are_removed() {
    let dir = temp_dir();
    let old = image(&dir, "src-digest-100.ext4", DAY * 5);
    let newest = image(&dir, "src-digest-200.ext4", DAY * 3);
    let recent_old = image(&dir, "src-other-100.ext4", DAY / 2);
    let recent_new = image(&dir, "src-other-200.ext4", Duration::from_secs(60));
    let alone = image(&dir, "lone-digest-100.ext4", DAY * 9);
    let unrelated = image(&dir, "notes.txt", DAY * 9);

    let removed = remove_superseded_rootfs(&dir, SystemTime::now()).await;

    assert_eq!(removed, 1);
    assert!(!old.exists());
    for kept in [newest, recent_old, recent_new, alone, unrelated] {
        assert!(kept.exists());
    }
    std::fs::remove_dir_all(dir).ok();
}
