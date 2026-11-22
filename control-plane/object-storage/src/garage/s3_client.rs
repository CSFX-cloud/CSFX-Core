use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    sign, PayloadChecksumKind, PercentEncodingMode, SignableBody, SignableRequest,
    SignatureLocation, SigningSettings,
};
use aws_sigv4::sign::v4;
use percent_encoding::AsciiSet;
use serde::Deserialize;
use std::time::{Duration, SystemTime};

const S3_REGION: &str = "csfx";
const S3_SERVICE: &str = "s3";
const EMPTY_BODY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

const SIGV4_UNRESERVED: &AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Clone)]
pub struct S3Client {
    s3_url: String,
    http: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct S3Object {
    pub key: String,
    pub size: i64,
    pub last_modified: String,
}

#[derive(Debug, Default)]
pub struct ListObjectsResult {
    pub objects: Vec<S3Object>,
    pub common_prefixes: Vec<String>,
    pub next_continuation_token: Option<String>,
}

impl S3Client {
    pub fn new(s3_url: String) -> Self {
        Self {
            s3_url,
            http: shared::http::client(shared::http::DEFAULT_TIMEOUT),
        }
    }

    fn identity(
        access_key_id: &str,
        secret_access_key: &str,
    ) -> aws_smithy_runtime_api::client::identity::Identity {
        Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "csfx-object-storage",
        )
        .into()
    }

    fn signing_params<'a>(
        identity: &'a aws_smithy_runtime_api::client::identity::Identity,
        settings: SigningSettings,
    ) -> Result<aws_sigv4::sign::v4::SigningParams<'a, SigningSettings>> {
        v4::SigningParams::builder()
            .identity(identity)
            .region(S3_REGION)
            .name(S3_SERVICE)
            .time(SystemTime::now())
            .settings(settings)
            .build()
            .context("failed to build sigv4 signing params")
    }

    pub async fn list_objects(
        &self,
        bucket: &str,
        access_key_id: &str,
        secret_access_key: &str,
        prefix: &str,
        delimiter: &str,
        continuation_token: Option<&str>,
    ) -> Result<ListObjectsResult> {
        let mut url = format!(
            "{}/{}?list-type=2&prefix={}&delimiter={}",
            self.s3_url,
            bucket,
            percent_encoding::utf8_percent_encode(prefix, SIGV4_UNRESERVED),
            percent_encoding::utf8_percent_encode(delimiter, SIGV4_UNRESERVED),
        );
        if let Some(token) = continuation_token {
            url.push_str(&format!(
                "&continuation-token={}",
                percent_encoding::utf8_percent_encode(token, SIGV4_UNRESERVED)
            ));
        }

        let identity = Self::identity(access_key_id, secret_access_key);
        let params = Self::signing_params(&identity, SigningSettings::default())?;

        let signable = SignableRequest::new(
            "GET",
            &url,
            std::iter::once(("x-amz-content-sha256", EMPTY_BODY_SHA256)),
            SignableBody::Bytes(&[]),
        )
        .context("failed to build signable request")?;

        let signed_headers: Vec<(String, String)> = sign(signable, &params.into())
            .context("failed to sign request")?
            .into_parts()
            .0
            .headers()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();

        let mut request = self
            .http
            .get(&url)
            .header("x-amz-content-sha256", EMPTY_BODY_SHA256)
            .build()
            .context("failed to build request")?;
        for (name, value) in signed_headers {
            request.headers_mut().insert(
                http::HeaderName::from_bytes(name.as_bytes())
                    .context("invalid signed header name")?,
                value.parse().context("invalid signed header value")?,
            );
        }

        let response = self
            .http
            .execute(request)
            .await
            .context("list_objects request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("list_objects failed status={} body={}", status, body);
        }

        let body = response
            .text()
            .await
            .context("failed to read list_objects response")?;
        parse_list_objects_xml(&body)
    }

    pub async fn delete_object(
        &self,
        bucket: &str,
        access_key_id: &str,
        secret_access_key: &str,
        key: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/{}/{}",
            self.s3_url,
            bucket,
            percent_encoding::utf8_percent_encode(key, SIGV4_UNRESERVED)
        );

        let identity = Self::identity(access_key_id, secret_access_key);
        let mut settings = SigningSettings::default();
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        let params = Self::signing_params(&identity, settings)?;

        let signable = SignableRequest::new(
            "DELETE",
            &url,
            std::iter::once(("x-amz-content-sha256", EMPTY_BODY_SHA256)),
            SignableBody::Bytes(&[]),
        )
        .context("failed to build signable request")?;

        let signed_headers: Vec<(String, String)> = sign(signable, &params.into())
            .context("failed to sign request")?
            .into_parts()
            .0
            .headers()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();

        let mut request = self
            .http
            .delete(&url)
            .header("x-amz-content-sha256", EMPTY_BODY_SHA256)
            .build()
            .context("failed to build request")?;
        for (name, value) in signed_headers {
            request.headers_mut().insert(
                http::HeaderName::from_bytes(name.as_bytes())
                    .context("invalid signed header name")?,
                value.parse().context("invalid signed header value")?,
            );
        }

        let response = self
            .http
            .execute(request)
            .await
            .context("delete_object request failed")?;

        if !response.status().is_success() && response.status().as_u16() != 404 {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("delete_object failed status={} body={}", status, body);
        }

        Ok(())
    }

    async fn send_signed(
        &self,
        method: reqwest::Method,
        bucket: &str,
        key: &str,
        access_key_id: &str,
        secret_access_key: &str,
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        let url = format!("{}/{}/{}", self.s3_url, bucket, encode_key(key));
        let identity = Self::identity(access_key_id, secret_access_key);
        let mut settings = SigningSettings::default();
        settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        let params = Self::signing_params(&identity, settings)?;
        let signable = SignableRequest::new(
            method.as_str(),
            &url,
            std::iter::empty(),
            SignableBody::Bytes(&body),
        )
        .context("failed to build signable request")?;
        let (instructions, _) = sign(signable, &params.into())
            .context("failed to sign request")?
            .into_parts();
        let mut request = self.http.request(method.clone(), &url);
        for (name, value) in instructions.headers() {
            request = request.header(name, value);
        }
        let response = request
            .body(body)
            .send()
            .await
            .with_context(|| format!("{method} object request failed"))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        anyhow::bail!("{} object failed status={} body={}", method, status, detail)
    }

    pub async fn get_object(
        &self,
        bucket: &str,
        access_key_id: &str,
        secret_access_key: &str,
        key: &str,
    ) -> Result<reqwest::Response> {
        self.send_signed(
            reqwest::Method::GET,
            bucket,
            key,
            access_key_id,
            secret_access_key,
            Vec::new(),
        )
        .await
    }

    pub async fn put_object(
        &self,
        bucket: &str,
        access_key_id: &str,
        secret_access_key: &str,
        key: &str,
        body: Vec<u8>,
    ) -> Result<()> {
        self.send_signed(
            reqwest::Method::PUT,
            bucket,
            key,
            access_key_id,
            secret_access_key,
            body,
        )
        .await
        .map(drop)
    }

    pub fn presign_url(
        &self,
        public_origin: &str,
        method: &str,
        bucket: &str,
        key: &str,
        access_key_id: &str,
        secret_access_key: &str,
        expires_in: Duration,
    ) -> Result<String> {
        let url = format!(
            "{}/{}/{}",
            public_origin,
            bucket,
            percent_encoding::utf8_percent_encode(key, SIGV4_UNRESERVED)
        );

        let identity = Self::identity(access_key_id, secret_access_key);
        let mut settings = SigningSettings::default();
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        settings.signature_location = SignatureLocation::QueryParams;
        settings.expires_in = Some(expires_in);
        let params = Self::signing_params(&identity, settings)?;

        let signable = SignableRequest::new(
            method,
            &url,
            std::iter::empty(),
            SignableBody::UnsignedPayload,
        )
        .context("failed to build signable request")?;

        let (instructions, _) = sign(signable, &params.into())
            .context("failed to sign presigned url")?
            .into_parts();

        let mut request = http::Request::builder()
            .method(method)
            .uri(&url)
            .body(())
            .context("failed to build presign request")?;
        instructions.apply_to_request_http1x(&mut request);

        let path_and_query = request
            .uri()
            .path_and_query()
            .context("presigned uri has no path")?
            .as_str();

        Ok(format!("{public_origin}/s3data{path_and_query}"))
    }
}

fn parse_list_objects_xml(body: &str) -> Result<ListObjectsResult> {
    let mut result = ListObjectsResult::default();

    for segment in body.split("<Contents>").skip(1) {
        let end = segment.find("</Contents>").unwrap_or(segment.len());
        let entry = &segment[..end];
        let key = extract_xml_tag(entry, "Key")
            .map(|raw| unescape_xml(&raw))
            .unwrap_or_default();
        let size = extract_xml_tag(entry, "Size")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let last_modified = extract_xml_tag(entry, "LastModified").unwrap_or_default();
        if !key.is_empty() {
            result.objects.push(S3Object {
                key,
                size,
                last_modified,
            });
        }
    }

    for segment in body.split("<CommonPrefixes>").skip(1) {
        let end = segment.find("</CommonPrefixes>").unwrap_or(segment.len());
        let entry = &segment[..end];
        if let Some(prefix) = extract_xml_tag(entry, "Prefix") {
            result.common_prefixes.push(prefix);
        }
    }

    result.next_continuation_token = extract_xml_tag(body, "NextContinuationToken");

    Ok(result)
}

fn extract_xml_tag(input: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = input.find(&open)? + open.len();
    let end = input[start..].find(&close)? + start;
    Some(input[start..end].to_string())
}

fn encode_key(key: &str) -> String {
    key.split('/')
        .map(|segment| percent_encoding::utf8_percent_encode(segment, SIGV4_UNRESERVED).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn unescape_xml(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_keys_are_unescaped() {
        let body = "<ListBucketResult><Contents><Key>a&amp;b&lt;c&gt;/d&quot;e&apos;f</Key><Size>3</Size><LastModified>x</LastModified></Contents></ListBucketResult>";
        let result = parse_list_objects_xml(body).unwrap();
        assert_eq!(result.objects[0].key, "a&b<c>/d\"e'f");
    }

    #[test]
    fn keys_keep_their_slashes_and_encode_segments() {
        assert_eq!(encode_key("dir/a b/c&d.txt"), "dir/a%20b/c%26d.txt");
        assert_eq!(encode_key("flat"), "flat");
    }

    #[test]
    fn ampersand_entities_are_not_double_decoded() {
        assert_eq!(unescape_xml("&amp;lt;"), "&lt;");
    }
}
