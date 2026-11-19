use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

const HEADERS: [(&str, &str); 6] = [
    (
        "content-security-policy",
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ws: wss:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
    ),
    ("strict-transport-security", "max-age=31536000; includeSubDomains"),
    ("x-frame-options", "DENY"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("permissions-policy", "camera=(), microphone=(), geolocation=()"),
];

pub async fn apply(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, middleware::from_fn, routing::get, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn every_response_carries_the_security_headers() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(from_fn(apply));
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        for (name, value) in HEADERS {
            assert_eq!(response.headers()[name], value);
        }
    }
}
