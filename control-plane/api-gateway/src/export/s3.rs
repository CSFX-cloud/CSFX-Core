use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    sign, PayloadChecksumKind, PercentEncodingMode, SignableBody, SignableRequest, SigningSettings,
};
use aws_sigv4::sign::v4;
use axum::body::Bytes;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::{Method, Response};
use std::time::SystemTime;

use super::sink::SinkError;

const SERVICE: &str = "s3";
const QUERY_ENCODE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Clone, Debug)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Clone)]
pub struct S3Sink {
    config: S3Config,
    http: reqwest::Client,
}

fn tag(body: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = body.find(&open)? + open.len();
    let end = body[start..].find(&format!("</{name}>"))? + start;
    Some(body[start..end].to_string())
}

impl S3Sink {
    pub fn new(config: S3Config) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    fn object_key(&self, key: &str) -> String {
        let prefix = self.config.prefix.trim_matches('/');
        if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}/{key}")
        }
    }

    fn url(&self, key: &str, query: &str) -> String {
        let base = format!(
            "{}/{}/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.bucket,
            self.object_key(key)
        );
        if query.is_empty() {
            base
        } else {
            format!("{base}?{query}")
        }
    }

    fn signed_headers(
        &self,
        method: &str,
        url: &str,
        body: &[u8],
    ) -> Result<Vec<(String, String)>, SinkError> {
        let identity = Credentials::new(
            &self.config.access_key_id,
            &self.config.secret_access_key,
            None,
            None,
            "csfx-export",
        )
        .into();
        let mut settings = SigningSettings::default();
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
        let params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.config.region)
            .name(SERVICE)
            .time(SystemTime::now())
            .settings(settings)
            .build()
            .map_err(|e| SinkError::Remote(e.to_string()))?;
        let signable =
            SignableRequest::new(method, url, std::iter::empty(), SignableBody::Bytes(body))
                .map_err(|e| SinkError::Remote(e.to_string()))?;
        let (instructions, _) = sign(signable, &params.into())
            .map_err(|e| SinkError::Remote(e.to_string()))?
            .into_parts();
        Ok(instructions
            .headers()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect())
    }

    async fn send(
        &self,
        method: Method,
        key: &str,
        query: &str,
        body: Bytes,
    ) -> Result<Response, SinkError> {
        let url = self.url(key, query);
        let mut request = self.http.request(method.clone(), &url);
        for (name, value) in self.signed_headers(method.as_str(), &url, &body)? {
            request = request.header(name, value);
        }
        let response = request.body(body).send().await?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        Err(SinkError::Remote(format!(
            "{method} {key} failed status={status} body={detail}"
        )))
    }

    pub async fn put(&self, key: &str, body: Bytes) -> Result<(), SinkError> {
        self.send(Method::PUT, key, "", body).await.map(drop)
    }

    pub async fn get(&self, key: &str) -> Result<Response, SinkError> {
        self.send(Method::GET, key, "", Bytes::new()).await
    }

    pub async fn delete(&self, key: &str) -> Result<(), SinkError> {
        self.send(Method::DELETE, key, "", Bytes::new())
            .await
            .map(drop)
    }

    pub async fn start_upload(&self, key: &str) -> Result<String, SinkError> {
        let response = self
            .send(Method::POST, key, "uploads", Bytes::new())
            .await?;
        let body = response.text().await?;
        tag(&body, "UploadId").ok_or_else(|| SinkError::Remote("missing upload id".to_string()))
    }

    pub async fn upload_part(
        &self,
        key: &str,
        upload_id: &str,
        number: u32,
        body: Bytes,
    ) -> Result<String, SinkError> {
        let query = format!(
            "partNumber={number}&uploadId={}",
            utf8_percent_encode(upload_id, QUERY_ENCODE)
        );
        let response = self.send(Method::PUT, key, &query, body).await?;
        response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .ok_or_else(|| SinkError::Remote("part response has no etag".to_string()))
    }

    pub async fn complete_upload(
        &self,
        key: &str,
        upload_id: &str,
        etags: &[String],
    ) -> Result<(), SinkError> {
        let parts: String = etags
            .iter()
            .enumerate()
            .map(|(index, etag)| {
                format!(
                    "<Part><PartNumber>{}</PartNumber><ETag>{etag}</ETag></Part>",
                    index + 1
                )
            })
            .collect();
        let body = format!("<CompleteMultipartUpload>{parts}</CompleteMultipartUpload>");
        let query = format!("uploadId={}", utf8_percent_encode(upload_id, QUERY_ENCODE));
        self.send(Method::POST, key, &query, Bytes::from(body))
            .await
            .map(drop)
    }

    pub async fn abort_upload(&self, key: &str, upload_id: &str) -> Result<(), SinkError> {
        let query = format!("uploadId={}", utf8_percent_encode(upload_id, QUERY_ENCODE));
        self.send(Method::DELETE, key, &query, Bytes::new())
            .await
            .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink(prefix: &str) -> S3Sink {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        S3Sink::new(S3Config {
            endpoint: "https://s3.example.com/".to_string(),
            region: "eu-central-1".to_string(),
            bucket: "backups".to_string(),
            prefix: prefix.to_string(),
            access_key_id: "id".to_string(),
            secret_access_key: "secret".to_string(),
        })
    }

    #[test]
    fn builds_path_style_urls_with_prefix() {
        assert_eq!(
            sink("/csfx/prod/").url("abc/manifest.json", ""),
            "https://s3.example.com/backups/csfx/prod/abc/manifest.json"
        );
        assert_eq!(
            sink("").url("abc/x", "uploads"),
            "https://s3.example.com/backups/abc/x?uploads"
        );
    }

    #[test]
    fn extracts_upload_id() {
        let body = "<InitiateMultipartUploadResult><UploadId>abc-123</UploadId></InitiateMultipartUploadResult>";
        assert_eq!(tag(body, "UploadId").as_deref(), Some("abc-123"));
        assert_eq!(tag(body, "Missing"), None);
    }
}
