use axum::{
    extract::{OriginalUri, Request},
    http::Method,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::auth::{jwt::verify_jwt, middleware::request_token};

const API_PREFIX: &str = "/api/";

struct AuditTarget<'a> {
    resource: &'a str,
    id: Option<&'a str>,
    verb: &'a str,
}

fn is_mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn default_verb(method: &Method) -> &'static str {
    match *method {
        Method::DELETE => "delete",
        Method::POST => "create",
        _ => "update",
    }
}

fn parse_target<'a>(path: &'a str, method: &Method) -> Option<AuditTarget<'a>> {
    let mut segments = path.strip_prefix(API_PREFIX)?.split('/').filter(|s| !s.is_empty());
    let resource = segments.next()?;
    let mut id = None;
    let mut verb = default_verb(method);
    for segment in segments {
        if Uuid::parse_str(segment).is_ok() {
            id = id.or(Some(segment));
        } else {
            verb = segment;
        }
    }
    Some(AuditTarget { resource, id, verb })
}

pub async fn audit_user_actions(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let actor = is_mutating(&method)
        .then(|| request_token(request.headers()).and_then(|t| verify_jwt(t).ok()))
        .flatten()
        .map(|data| data.claims.user_id);
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map(|uri| uri.0.path().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let response = next.run(request).await;

    let Some(actor) = actor else {
        return response;
    };
    if !response.status().is_success() {
        return response;
    }
    if let Some(target) = parse_target(&path, &method) {
        shared::audit!(
            actor,
            format!("{}.{}", target.resource, target.verb),
            target.resource,
            target.id.unwrap_or("-")
        );
    }
    response
}
