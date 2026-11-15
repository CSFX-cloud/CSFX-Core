use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::LazyLock;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to build dashboard proxy client")
});

const HOP_BY_HOP: [HeaderName; 6] = [
    header::CONNECTION,
    header::TRANSFER_ENCODING,
    header::UPGRADE,
    header::TE,
    header::TRAILER,
    header::HOST,
];

fn forwardable(headers: &HeaderMap) -> HeaderMap {
    let mut forwarded = headers.clone();
    for name in &HOP_BY_HOP {
        forwarded.remove(name);
    }
    forwarded
}

fn forwarded_origin_headers(headers: &HeaderMap, forwarded: &mut HeaderMap) {
    let origin = crate::oidc::public_origin(headers);
    let Some((scheme, host)) = origin.split_once("://") else {
        return;
    };
    for (name, value) in [("x-forwarded-proto", scheme), ("x-forwarded-host", host)] {
        if let Ok(value) = value.parse() {
            forwarded.insert(HeaderName::from_static(name), value);
        }
    }
}

pub async fn forward(base_url: String, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map_or("/", |value| value.as_str());
    let url = format!("{}{}", base_url.trim_end_matches('/'), path_and_query);
    let mut headers = forwardable(&parts.headers);
    forwarded_origin_headers(&parts.headers, &mut headers);

    let upstream = CLIENT
        .request(parts.method, &url)
        .headers(headers)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await;

    match upstream {
        Ok(reply) => relay(reply),
        Err(error) => {
            tracing::error!(error = %error, url = %url, "dashboard upstream unavailable");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

fn relay(reply: reqwest::Response) -> Response {
    let status = reply.status();
    let headers = forwardable(reply.headers());
    let mut response = Response::new(Body::from_stream(reply.bytes_stream()));
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::HeaderValue, routing::get, Router};

    fn install_crypto_provider() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
    }

    async fn spawn_upstream() -> String {
        let upstream = Router::new().route(
            "/nodes",
            get(|headers: HeaderMap| async move {
                let forwarded = headers
                    .get("x-forwarded-host")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                (
                    StatusCode::SEE_OTHER,
                    [
                        (header::LOCATION, "/login?next=%2Fnodes".to_string()),
                        (header::SET_COOKIE, forwarded),
                    ],
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn relays_redirects_without_following_them() {
        install_crypto_provider();
        let base_url = spawn_upstream().await;
        let request = Request::builder()
            .uri("/nodes")
            .header(header::HOST, "dashboard.test")
            .body(Body::empty())
            .unwrap();

        let response = forward(base_url, request).await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(header::LOCATION),
            Some(&HeaderValue::from_static("/login?next=%2Fnodes"))
        );
        assert_eq!(
            response.headers().get(header::SET_COOKIE),
            Some(&HeaderValue::from_static("dashboard.test"))
        );
    }

    #[tokio::test]
    async fn answers_bad_gateway_when_the_dashboard_is_down() {
        install_crypto_provider();
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = forward("http://127.0.0.1:1".to_string(), request).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }
}
