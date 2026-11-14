use crate::AppState;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

const PROTOCOL_HEADER: &str = "X-Csfx-Agent-Protocol";
pub const MIN_AGENT_PROTOCOL: u32 = 1;

pub struct AgentApiKey {
    pub agent_id: Uuid,
}

pub enum AgentRejection {
    Status(StatusCode),
    Protocol(StatusCode, &'static str),
}

impl From<StatusCode> for AgentRejection {
    fn from(status: StatusCode) -> Self {
        Self::Status(status)
    }
}

impl IntoResponse for AgentRejection {
    fn into_response(self) -> Response {
        match self {
            Self::Status(status) => status.into_response(),
            Self::Protocol(status, message) => (status, message).into_response(),
        }
    }
}

fn check_protocol(headers: &HeaderMap) -> Result<(), AgentRejection> {
    let Some(raw) = headers.get(PROTOCOL_HEADER) else {
        return Ok(());
    };
    let version = raw
        .to_str()
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok());
    match version {
        None => Err(AgentRejection::Protocol(
            StatusCode::BAD_REQUEST,
            "invalid agent protocol header",
        )),
        Some(version) if version < MIN_AGENT_PROTOCOL => Err(AgentRejection::Protocol(
            StatusCode::UPGRADE_REQUIRED,
            "agent protocol version is too old, upgrade the agent",
        )),
        Some(_) => Ok(()),
    }
}

impl FromRequestParts<AppState> for AgentApiKey {
    type Rejection = AgentRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        check_protocol(&parts.headers)?;
        let raw_key = parts
            .headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let agent_id = shared::agent_auth::agent_for_key(&state.agent_db, raw_key)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(AgentApiKey { agent_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(value: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(value) = value {
            headers.insert(PROTOCOL_HEADER, value.parse().unwrap());
        }
        headers
    }

    fn status(result: Result<(), AgentRejection>) -> Option<StatusCode> {
        match result {
            Err(AgentRejection::Protocol(status, _)) => Some(status),
            _ => None,
        }
    }

    #[test]
    fn missing_header_is_accepted_for_old_agents() {
        assert!(check_protocol(&headers(None)).is_ok());
    }

    #[test]
    fn current_and_newer_versions_are_accepted() {
        assert!(check_protocol(&headers(Some("1"))).is_ok());
        assert!(check_protocol(&headers(Some("7"))).is_ok());
    }

    #[test]
    fn versions_below_the_minimum_need_an_upgrade() {
        assert_eq!(
            status(check_protocol(&headers(Some("0")))),
            Some(StatusCode::UPGRADE_REQUIRED)
        );
    }

    #[test]
    fn malformed_versions_are_rejected() {
        assert_eq!(
            status(check_protocol(&headers(Some("abc")))),
            Some(StatusCode::BAD_REQUEST)
        );
    }
}
