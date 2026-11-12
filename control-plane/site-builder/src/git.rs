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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoPolicy {
    PublicOnly,
    Unrestricted,
}

const CLONE_FAILED: &str = "clone failed";
const DEFAULT_HTTPS_PORT: u16 = 443;

fn validate(request: &SnapshotRequest, policy: RepoPolicy) -> Result<(), ServiceError> {
    let location = match policy {
        RepoPolicy::PublicOnly => crate::validate::repo_url(&request.repo_url),
        RepoPolicy::Unrestricted => crate::validate::repo_url_syntax(&request.repo_url),
    };
    location
        .and_then(|()| crate::validate::branch(&request.branch))
        .and_then(|()| crate::validate::root_dir(&request.root_dir))
        .map_err(|message| ServiceError::Invalid(message.to_string()))
}

fn auth_header(token: &str) -> String {
    let credentials = STANDARD.encode(format!("{TOKEN_USER}:{token}"));
    format!("Authorization: Basic {credentials}")
}

fn clone_command(
    request: &SnapshotRequest,
    target: &Path,
    settings: &[(String, String)],
    protocols: &str,
) -> Command {
    let mut command = Command::new("git");
    command
        .args(["clone", "--depth", "1", "--single-branch", "--branch"])
        .arg(&request.branch)
        .arg("--")
        .arg(&request.repo_url)
        .arg(target)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ALLOW_PROTOCOL", protocols)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command.env("GIT_CONFIG_COUNT", settings.len().to_string());
    for (index, (key, value)) in settings.iter().enumerate() {
        command
            .env(format!("GIT_CONFIG_KEY_{index}"), key)
            .env(format!("GIT_CONFIG_VALUE_{index}"), value);
    }
    command
}

fn git_settings(request: &SnapshotRequest, pinned: &[String]) -> Vec<(String, String)> {
    let mut settings = vec![("http.followRedirects".to_string(), "false".to_string())];
    settings.extend(
        pinned
            .iter()
            .map(|entry| ("http.curloptResolve".to_string(), entry.clone())),
    );
    if let Some(token) = &request.token {
        settings.push(("http.extraHeader".to_string(), auth_header(token)));
    }
    settings
}

async fn resolve_public(url: &str) -> Result<Vec<String>, ServiceError> {
    let not_allowed = || ServiceError::Invalid(crate::validate::REPOSITORY_NOT_ALLOWED.to_string());
    let url = reqwest::Url::parse(url).map_err(|_| not_allowed())?;
    let host = url.host_str().ok_or_else(not_allowed)?;
    let port = url.port().unwrap_or(DEFAULT_HTTPS_PORT);
    let literal = host.trim_start_matches('[').trim_end_matches(']');
    if literal.parse::<std::net::IpAddr>().is_ok() {
        return Ok(Vec::new());
    }
    let addresses: Vec<std::net::IpAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| not_allowed())?
        .map(|address| address.ip())
        .collect();
    if addresses.is_empty()
        || !addresses
            .iter()
            .all(|a| crate::validate::is_public_address(*a))
    {
        return Err(not_allowed());
    }
    let list: Vec<String> = addresses
        .iter()
        .map(|address| match address {
            std::net::IpAddr::V4(v4) => v4.to_string(),
            std::net::IpAddr::V6(v6) => format!("[{v6}]"),
        })
        .collect();
    Ok(vec![format!("{host}:{port}:{}", list.join(","))])
}

async fn clone(
    request: &SnapshotRequest,
    target: &Path,
    policy: RepoPolicy,
) -> Result<(), ServiceError> {
    let (pinned, protocols) = match policy {
        RepoPolicy::PublicOnly => (resolve_public(&request.repo_url).await?, "https"),
        RepoPolicy::Unrestricted => (Vec::new(), "http:https"),
    };
    let settings = git_settings(request, &pinned);
    let run = clone_command(request, target, &settings, protocols).output();
    let output = tokio::time::timeout(CLONE_TIMEOUT, run)
        .await
        .map_err(|_| ServiceError::Git("clone timed out".to_string()))?
        .map_err(|error| {
            tracing::warn!(error = %error, "git could not be started");
            ServiceError::Git(CLONE_FAILED.to_string())
        })?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    let redacted = request
        .token
        .as_deref()
        .map_or_else(|| detail.to_string(), |token| detail.replace(token, "***"));
    tracing::warn!(detail = %redacted.trim(), "git clone failed");
    Err(ServiceError::Git(CLONE_FAILED.to_string()))
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
    snapshot_with(request, RepoPolicy::PublicOnly).await
}

pub async fn snapshot_with(
    request: &SnapshotRequest,
    policy: RepoPolicy,
) -> Result<RepoSnapshot, ServiceError> {
    validate(request, policy)?;
    let target = scratch_dir();
    let outcome = async {
        clone(request, &target, policy).await?;
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
        assert!(matches!(
            validate(&request, RepoPolicy::PublicOnly),
            Err(ServiceError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn internal_targets_are_refused_without_any_connection() {
        for url in [
            "http://127.0.0.1:2379/x",
            "https://127.0.0.1:2379/x",
            "https://10.0.0.5/x.git",
            "https://localhost/x.git",
        ] {
            let request = SnapshotRequest {
                repo_url: url.to_string(),
                branch: "main".to_string(),
                root_dir: String::new(),
                token: Some("tok".to_string()),
            };
            let Err(error) = snapshot(&request).await else {
                panic!("{url} must be refused");
            };
            let message = error.to_string();
            assert!(
                !message.contains("127.0.0.1") && !message.contains("tok"),
                "{message}"
            );
            let expected = if url.starts_with("http://") {
                "repository url must start with https://"
            } else {
                "repository url is not allowed"
            };
            assert_eq!(message, expected, "{url}");
        }
    }

    #[test]
    fn clone_settings_block_redirects_and_carry_the_token_only_when_given() {
        let request = SnapshotRequest {
            repo_url: "https://example.com/a.git".to_string(),
            branch: "main".to_string(),
            root_dir: String::new(),
            token: None,
        };
        let pinned = vec!["example.com:443:93.184.216.34".to_string()];
        let settings = git_settings(&request, &pinned);
        assert!(settings.contains(&("http.followRedirects".to_string(), "false".to_string())));
        assert!(settings.iter().any(|(key, _)| key == "http.curloptResolve"));
        assert!(!settings.iter().any(|(key, _)| key == "http.extraHeader"));
        let with_token = SnapshotRequest {
            token: Some("tok".to_string()),
            ..request
        };
        assert!(git_settings(&with_token, &pinned)
            .iter()
            .any(|(key, _)| key == "http.extraHeader"));
    }
}
