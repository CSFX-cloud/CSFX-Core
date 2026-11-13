use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

pub static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

const MAX_REQUEST_ID_LENGTH: usize = 128;

tokio::task_local! {
    static CURRENT: HeaderValue;
}

pub fn current_request_id() -> Option<HeaderValue> {
    CURRENT.try_with(HeaderValue::clone).ok()
}

fn is_acceptable(value: &HeaderValue) -> bool {
    !value.is_empty() && value.len() <= MAX_REQUEST_ID_LENGTH && value.to_str().is_ok()
}

fn generate() -> HeaderValue {
    HeaderValue::from_str(&Uuid::new_v4().to_string()).expect("uuid is a valid header value")
}

pub async fn propagate(mut request: Request, next: Next) -> Response {
    let id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .filter(|value| is_acceptable(value))
        .cloned()
        .unwrap_or_else(generate);
    request
        .headers_mut()
        .insert(REQUEST_ID_HEADER.clone(), id.clone());

    let span = tracing::info_span!("request", request_id = id.to_str().unwrap_or_default());
    let mut response = CURRENT
        .scope(id.clone(), next.run(request).instrument(span))
        .await;
    response.headers_mut().insert(REQUEST_ID_HEADER.clone(), id);
    response
}
