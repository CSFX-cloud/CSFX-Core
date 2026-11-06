use anyhow::{bail, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::info;

pub async fn sync(mirror_dir: &str, remote_url: &str) -> Result<()> {
    if Path::new(mirror_dir).join("HEAD").exists() {
        fetch(mirror_dir).await
    } else {
        clone(mirror_dir, remote_url).await
    }
}

async fn clone(mirror_dir: &str, remote_url: &str) -> Result<()> {
    info!(mirror_dir = %mirror_dir, remote_url = %remote_url, "cloning infra repo mirror");

    let status = Command::new("git")
        .args(["clone", "--mirror", remote_url, mirror_dir])
        .status()
        .await?;

    if !status.success() {
        bail!("git clone --mirror failed for {}", remote_url);
    }

    info!(mirror_dir = %mirror_dir, "mirror clone complete");
    Ok(())
}

async fn fetch(mirror_dir: &str) -> Result<()> {
    info!(mirror_dir = %mirror_dir, "fetching infra repo mirror");

    let status = Command::new("git")
        .args(["--git-dir", mirror_dir, "fetch", "--prune"])
        .status()
        .await?;

    if !status.success() {
        bail!("git fetch --prune failed in {}", mirror_dir);
    }

    info!(mirror_dir = %mirror_dir, "mirror fetch complete");
    Ok(())
}

pub async fn rev_exists(mirror_dir: &str, rev: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["--git-dir", mirror_dir, "cat-file", "-t", rev])
        .output()
        .await?;

    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "commit")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command as StdCommand;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("csfx-updater-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = StdCommand::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@example.invalid",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn origin_with_commit(root: &Path) -> (PathBuf, String) {
        let origin = root.join("origin");
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "-q"]);
        std::fs::write(origin.join("file.txt"), "content").unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-q", "-m", "initial"]);
        let sha = git(&origin, &["rev-parse", "HEAD"]);
        (origin, sha)
    }

    #[tokio::test]
    async fn sync_clones_then_fetches_new_commits() {
        let root = scratch_dir("sync");
        let (origin, first) = origin_with_commit(&root);
        let mirror = root.join("mirror.git");
        let mirror_dir = mirror.to_str().unwrap();
        let origin_url = origin.to_str().unwrap();

        sync(mirror_dir, origin_url).await.unwrap();
        assert!(mirror.join("HEAD").exists());
        assert!(rev_exists(mirror_dir, &first).await.unwrap());

        std::fs::write(origin.join("file.txt"), "changed").unwrap();
        git(&origin, &["commit", "-q", "-am", "second"]);
        let second = git(&origin, &["rev-parse", "HEAD"]);
        assert!(!rev_exists(mirror_dir, &second).await.unwrap());

        sync(mirror_dir, origin_url).await.unwrap();
        assert!(rev_exists(mirror_dir, &second).await.unwrap());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn unknown_revisions_and_missing_remotes_are_reported() {
        let root = scratch_dir("missing");
        let mirror = root.join("mirror.git");
        let missing = root.join("absent");

        let error = sync(mirror.to_str().unwrap(), missing.to_str().unwrap()).await;
        assert!(error.is_err());

        let (origin, _) = origin_with_commit(&root);
        sync(mirror.to_str().unwrap(), origin.to_str().unwrap())
            .await
            .unwrap();
        let unknown = "0".repeat(40);
        assert!(!rev_exists(mirror.to_str().unwrap(), &unknown)
            .await
            .unwrap());
        std::fs::remove_dir_all(&root).ok();
    }
}
