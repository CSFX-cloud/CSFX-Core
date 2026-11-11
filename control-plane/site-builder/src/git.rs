use crate::detect::RepoSnapshot;
use crate::error::ServiceError;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use uuid::Uuid;

const CLONE_TIMEOUT: Duration = Duration::from_secs(120);
const PACKAGE_MANIFEST: &str = "package.json";
const TOKEN_USER: &str = "x-access-token";

#[derive(Debug, Deserialize)]
pub struct SnapshotRequest {
    pub repo_url: String,
    pub branch: String,
    #[serde(default)]
    pub root_dir: String,
    pub token: Option<String>,
}

fn validate(request: &SnapshotRequest) -> Result<(), ServiceError> {
    crate::validate::repo_url(&request.repo_url)
        .and_then(|()| crate::validate::branch(&request.branch))
        .and_then(|()| crate::validate::root_dir(&request.root_dir))
        .map_err(|message| ServiceError::Invalid(message.to_string()))
}

fn auth_header(token: &str) -> String {
    let credentials = STANDARD.encode(format!("{TOKEN_USER}:{token}"));
    format!("Authorization: Basic {credentials}")
}

fn clone_command(request: &SnapshotRequest, target: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .args(["clone", "--depth", "1", "--single-branch", "--branch"])
        .arg(&request.branch)
        .arg("--")
        .arg(&request.repo_url)
        .arg(target)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(token) = &request.token {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.extraHeader")
            .env("GIT_CONFIG_VALUE_0", auth_header(token));
    }
    command
}

async fn clone(request: &SnapshotRequest, target: &Path) -> Result<(), ServiceError> {
    let run = clone_command(request, target).output();
    let output = tokio::time::timeout(CLONE_TIMEOUT, run)
        .await
        .map_err(|_| ServiceError::Git("clone timed out".to_string()))?
        .map_err(|error| ServiceError::Git(error.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    let redacted = request
        .token
        .as_deref()
        .map_or_else(|| detail.to_string(), |token| detail.replace(token, "***"));
    Err(ServiceError::Git(redacted.trim().to_string()))
}

async fn read_snapshot(root: &Path) -> Result<RepoSnapshot, ServiceError> {
    let mut files = HashSet::new();
    let mut entries = tokio::fs::read_dir(root)
        .await
        .map_err(|_| ServiceError::Invalid("root directory does not exist".to_string()))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| ServiceError::Git(error.to_string()))?
    {
        files.insert(entry.file_name().to_string_lossy().into_owned());
    }
    let package_json = match tokio::fs::read(root.join(PACKAGE_MANIFEST)).await {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes).ok(),
        Err(_) => None,
    };
    Ok(RepoSnapshot {
        files,
        package_json,
    })
}

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join(format!("csfx-site-{}", Uuid::new_v4()))
}

pub async fn snapshot(request: &SnapshotRequest) -> Result<RepoSnapshot, ServiceError> {
    validate(request)?;
    let target = scratch_dir();
    let outcome = async {
        clone(request, &target).await?;
        read_snapshot(&target.join(&request.root_dir)).await
    }
    .await;
    if let Err(error) = tokio::fs::remove_dir_all(&target).await {
        tracing::warn!(error = %error, path = %target.display(), "scratch cleanup failed");
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_header_uses_basic_credentials() {
        assert_eq!(
            auth_header("abc"),
            "Authorization: Basic eC1hY2Nlc3MtdG9rZW46YWJj"
        );
    }

    #[test]
    fn snapshot_rejects_unsafe_requests_before_cloning() {
        let request = SnapshotRequest {
            repo_url: "file:///etc".to_string(),
            branch: "main".to_string(),
            root_dir: String::new(),
            token: None,
        };
        assert!(matches!(validate(&request), Err(ServiceError::Invalid(_))));
    }
}
