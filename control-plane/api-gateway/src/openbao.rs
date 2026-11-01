use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{Certificate, Client, Identity, Method, StatusCode};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::RwLock;

const CREDENTIAL_CA: &str = "openbao-ca";
const CREDENTIAL_CLIENT_CERTIFICATE: &str = "openbao-client-certificate";
const CREDENTIAL_CLIENT_KEY: &str = "openbao-client-key";
const CREDENTIAL_ROLE_ID: &str = "openbao-role-id";
const CREDENTIAL_SECRET_ID: &str = "openbao-secret-id";
const ADDRESSES_VARIABLE: &str = "OPENBAO_ADDRS";
const VIP_VARIABLE: &str = "CSFX_VIP_ADDRESS";
const LOCAL_ADDRESS: &str = "https://127.0.0.1:8200";
const API_VERSION_PREFIX: &str = "/v1/";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(60);

#[derive(Debug, Error)]
pub enum OpenBaoError {
    #[error("openbao credential {name} could not be read: {source}")]
    Credential {
        name: &'static str,
        source: std::io::Error,
    },
    #[error("openbao client could not be built: {0}")]
    Client(#[from] reqwest::Error),
    #[error("openbao is not bootstrapped yet")]
    NotBootstrapped,
    #[error("no openbao node is reachable")]
    Unreachable,
    #[error("openbao rejected the request status={status} detail={detail}")]
    Rejected { status: u16, detail: String },
    #[error("openbao response could not be decoded")]
    Decode,
}

struct Session {
    token: String,
    expires_at: Instant,
}

struct Inner {
    client: Client,
    nodes: Vec<String>,
    role_id: String,
    secret_id: String,
    session: RwLock<Option<Session>>,
}

struct Call<'a> {
    method: &'a Method,
    path: &'a str,
    body: Option<&'a Value>,
    wrap_ttl: Option<&'a str>,
}

#[derive(Clone)]
pub struct OpenBao {
    inner: Arc<Inner>,
}

pub fn node_addresses(explicit: Option<&str>, vip: Option<&str>) -> Vec<String> {
    if let Some(list) = explicit.filter(|value| !value.trim().is_empty()) {
        return list
            .split(',')
            .map(|address| address.trim().to_string())
            .filter(|address| !address.is_empty())
            .collect();
    }
    let mut nodes = vec![LOCAL_ADDRESS.to_string()];
    if let Some(vip) = vip.filter(|value| !value.is_empty()) {
        nodes.push(format!("https://{vip}:8200"));
    }
    nodes
}

pub fn rejection_detail(body: &Value) -> String {
    body.get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default()
}

fn read_credential(directory: &Path, name: &'static str) -> Result<String, OpenBaoError> {
    std::fs::read_to_string(directory.join(name))
        .map(|value| value.trim().to_string())
        .map_err(|source| OpenBaoError::Credential { name, source })
}

fn build_client(directory: &Path) -> Result<Client, OpenBaoError> {
    let ca = read_credential(directory, CREDENTIAL_CA)?;
    let mut identity_pem = read_credential(directory, CREDENTIAL_CLIENT_CERTIFICATE)?;
    identity_pem.push('\n');
    identity_pem.push_str(&read_credential(directory, CREDENTIAL_CLIENT_KEY)?);
    Ok(Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .tls_certs_only([Certificate::from_pem(ca.as_bytes())?])
        .identity(Identity::from_pem(identity_pem.as_bytes())?)
        .build()?)
}

impl OpenBao {
    pub fn from_environment() -> Result<Self, OpenBaoError> {
        let directory = std::env::var("CREDENTIALS_DIRECTORY")
            .map(PathBuf::from)
            .map_err(|_| OpenBaoError::Credential {
                name: CREDENTIAL_CA,
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            })?;
        let nodes = node_addresses(
            std::env::var(ADDRESSES_VARIABLE).ok().as_deref(),
            std::env::var(VIP_VARIABLE).ok().as_deref(),
        );
        Ok(Self {
            inner: Arc::new(Inner {
                client: build_client(&directory)?,
                nodes,
                role_id: read_credential(&directory, CREDENTIAL_ROLE_ID)?,
                secret_id: read_credential(&directory, CREDENTIAL_SECRET_ID)?,
                session: RwLock::new(None),
            }),
        })
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Option<Value>, OpenBaoError> {
        self.authorized(Call {
            method: &method,
            path,
            body,
            wrap_ttl: None,
        })
        .await
    }

    pub async fn request_wrapped(
        &self,
        method: Method,
        path: &str,
        wrap_ttl: &str,
    ) -> Result<Option<Value>, OpenBaoError> {
        self.authorized(Call {
            method: &method,
            path,
            body: None,
            wrap_ttl: Some(wrap_ttl),
        })
        .await
    }

    pub async fn is_available(&self) -> bool {
        let call = Call {
            method: &Method::GET,
            path: "sys/health",
            body: None,
            wrap_ttl: None,
        };
        self.send(&call, None).await.is_ok()
    }

    async fn authorized(&self, call: Call<'_>) -> Result<Option<Value>, OpenBaoError> {
        let token = self.token().await?;
        match self.send(&call, Some(&token)).await {
            Err(OpenBaoError::Rejected { status: 403, .. }) => {
                *self.inner.session.write().await = None;
                let token = self.token().await?;
                self.send(&call, Some(&token)).await
            }
            outcome => outcome,
        }
    }

    async fn token(&self) -> Result<String, OpenBaoError> {
        if let Some(session) = self.inner.session.read().await.as_ref() {
            if session.expires_at > Instant::now() + TOKEN_REFRESH_MARGIN {
                return Ok(session.token.clone());
            }
        }
        self.login().await
    }

    async fn login(&self) -> Result<String, OpenBaoError> {
        if self.inner.role_id.is_empty() || self.inner.secret_id.is_empty() {
            return Err(OpenBaoError::NotBootstrapped);
        }
        let credentials = json!({
            "role_id": self.inner.role_id,
            "secret_id": self.inner.secret_id,
        });
        let call = Call {
            method: &Method::POST,
            path: "auth/approle/login",
            body: Some(&credentials),
            wrap_ttl: None,
        };
        let response = self.send(&call, None).await?.ok_or(OpenBaoError::Decode)?;
        let auth = response.get("auth").ok_or(OpenBaoError::Decode)?;
        let token = auth
            .get("client_token")
            .and_then(Value::as_str)
            .ok_or(OpenBaoError::Decode)?
            .to_string();
        let lease = auth
            .get("lease_duration")
            .and_then(Value::as_u64)
            .ok_or(OpenBaoError::Decode)?;
        *self.inner.session.write().await = Some(Session {
            token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(lease),
        });
        Ok(token)
    }

    async fn send(
        &self,
        call: &Call<'_>,
        token: Option<&str>,
    ) -> Result<Option<Value>, OpenBaoError> {
        let mut last_failure = OpenBaoError::Unreachable;
        for node in &self.inner.nodes {
            match self.send_to(node, call, token).await {
                Err(OpenBaoError::Unreachable) => {
                    tracing::warn!(node = %node, path = %call.path, "openbao node unreachable");
                    last_failure = OpenBaoError::Unreachable;
                }
                outcome => return outcome,
            }
        }
        Err(last_failure)
    }

    async fn send_to(
        &self,
        node: &str,
        call: &Call<'_>,
        token: Option<&str>,
    ) -> Result<Option<Value>, OpenBaoError> {
        let url = format!("{node}{API_VERSION_PREFIX}{}", call.path);
        let mut request = self.inner.client.request(call.method.clone(), url);
        if let Some(token) = token {
            request = request.header("X-Vault-Token", token);
        }
        if let Some(wrap_ttl) = call.wrap_ttl {
            request = request.header("X-Vault-Wrap-TTL", wrap_ttl);
        }
        if let Some(body) = call.body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|error| {
            tracing::warn!(node = %node, error = %error, "openbao request failed");
            OpenBaoError::Unreachable
        })?;
        interpret(response).await
    }
}

async fn interpret(response: reqwest::Response) -> Result<Option<Value>, OpenBaoError> {
    let status = response.status();
    if status == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    if status.is_server_error() {
        return Err(OpenBaoError::Unreachable);
    }
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(Some(body));
    }
    Err(OpenBaoError::Rejected {
        status: status.as_u16(),
        detail: rejection_detail(&body),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_addresses_are_split_and_trimmed() {
        let nodes = node_addresses(Some("https://a:8200, https://b:8200,"), Some("10.0.0.9"));
        assert_eq!(nodes, vec!["https://a:8200", "https://b:8200"]);
    }

    #[test]
    fn default_addresses_use_the_local_node_then_the_vip() {
        assert_eq!(
            node_addresses(None, Some("10.0.0.9")),
            vec!["https://127.0.0.1:8200", "https://10.0.0.9:8200"]
        );
        assert_eq!(
            node_addresses(Some("  "), None),
            vec!["https://127.0.0.1:8200"]
        );
    }

    #[test]
    fn rejection_detail_joins_error_messages() {
        let body = json!({ "errors": ["permission denied", "invalid token"] });
        assert_eq!(rejection_detail(&body), "permission denied; invalid token");
        assert_eq!(rejection_detail(&Value::Null), "");
    }
}
