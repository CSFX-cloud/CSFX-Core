use entity::identity_providers;
use serde_json::{Map, Value};

use super::error::OidcError;

#[derive(Debug, PartialEq, Eq)]
pub struct ExternalIdentity {
    pub subject: String,
    pub username: String,
    pub email: Option<String>,
    pub groups: Vec<String>,
}

fn string_claim(claims: &Map<String, Value>, name: &str) -> Option<String> {
    claims
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn group_claim(claims: &Map<String, Value>, name: &str) -> Vec<String> {
    match claims.get(name) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::String(single)) => vec![single.clone()],
        _ => Vec::new(),
    }
}

fn verified_email(
    claims: &Map<String, Value>,
    provider: &identity_providers::Model,
) -> Option<String> {
    let verified = claims.get("email_verified").and_then(Value::as_bool) == Some(true);
    if !verified {
        return None;
    }
    string_claim(claims, &provider.email_claim)
}

pub fn extract_identity(
    claims: &Map<String, Value>,
    provider: &identity_providers::Model,
) -> Result<ExternalIdentity, OidcError> {
    let subject =
        string_claim(claims, "sub").ok_or(OidcError::InvalidProvider("missing subject"))?;
    let email = verified_email(claims, provider);
    let username = string_claim(claims, &provider.username_claim)
        .or_else(|| email.clone())
        .unwrap_or_else(|| subject.clone());
    Ok(ExternalIdentity {
        subject,
        username,
        email,
        groups: group_claim(claims, &provider.groups_claim),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider() -> identity_providers::Model {
        identity_providers::Model {
            id: uuid::Uuid::nil(),
            organization_id: uuid::Uuid::nil(),
            slug: "authentik".into(),
            display_name: "Authentik".into(),
            issuer_url: "https://auth.example.com/application/o/csfx/".into(),
            client_id: "csfx".into(),
            client_secret_encrypted: String::new(),
            scopes: "openid profile email".into(),
            username_claim: "preferred_username".into(),
            email_claim: "email".into(),
            groups_claim: "groups".into(),
            default_role_id: None,
            auto_provision: true,
            enabled: true,
            created_at: chrono::Utc::now().naive_utc(),
        }
    }

    fn claims(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn extracts_groups_and_trusts_only_verified_email() {
        let verified = claims(json!({
            "sub": "abc", "preferred_username": "alice", "email": "a@x.io",
            "email_verified": true, "groups": ["ops", "dev"]
        }));
        let identity = extract_identity(&verified, &provider()).unwrap();
        assert_eq!(identity.username, "alice");
        assert_eq!(identity.email.as_deref(), Some("a@x.io"));
        assert_eq!(identity.groups, vec!["ops", "dev"]);

        let unverified = claims(json!({ "sub": "abc", "email": "a@x.io" }));
        let identity = extract_identity(&unverified, &provider()).unwrap();
        assert_eq!(identity.email, None);
        assert_eq!(identity.username, "abc");
    }
}
