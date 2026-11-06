use axum::http::{header, Method, StatusCode};
use percent_encoding::percent_decode_str;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::{create_role, Harness};
use crate::support_b::{
    full_harness, idp_code, idp_issuer, idp_mismatch_issuer, raw, IDP_CLIENT_SECRET,
};

const ORIGIN_HEADERS: [(&str, &str); 2] = [("host", "gw.test"), ("x-forwarded-proto", "http")];

fn slug() -> String {
    format!("idp-{}", &Uuid::new_v4().simple().to_string()[..16])
}

fn provider_body(slug: &str, role: Option<Uuid>) -> Value {
    json!({
        "slug": slug,
        "display_name": format!("Provider {slug}"),
        "issuer_url": idp_issuer(),
        "client_id": "csfx-client",
        "client_secret": IDP_CLIENT_SECRET,
        "default_role_id": role,
    })
}

async fn create_provider(harness: &Harness, body: Value) -> Value {
    let (status, created) = harness.post("/api/identity-providers", body).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    created
}

fn location(response: &crate::support_b::Raw) -> String {
    response.headers[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string()
}

fn query_param(url: &str, name: &str) -> String {
    let query = url.split_once('?').unwrap().1;
    let pair = query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{name}=")));
    percent_decode_str(pair.unwrap())
        .decode_utf8()
        .unwrap()
        .to_string()
}

async fn begin(harness: &Harness, slug: &str) -> (String, String) {
    let uri = format!("/api/auth/oidc/{slug}/start");
    let response = raw(harness, Method::GET, &uri, &ORIGIN_HEADERS, None).await;
    assert!(response.status.is_redirection());
    let target = location(&response);
    assert!(target.contains("/authorize?"), "{target}");
    (query_param(&target, "state"), query_param(&target, "nonce"))
}

async fn callback(harness: &Harness, slug: &str, code: &str, state: &str) -> String {
    let encoded = code.replace('~', "%7E").replace('@', "%40");
    let uri = format!("/api/auth/oidc/{slug}/callback?code={encoded}&state={state}");
    let response = raw(harness, Method::GET, &uri, &ORIGIN_HEADERS, None).await;
    assert!(response.status.is_redirection());
    location(&response)
}

async fn login(
    harness: &Harness,
    slug: &str,
    subject: &str,
    username: &str,
    groups: &str,
) -> String {
    let (state, nonce) = begin(harness, slug).await;
    callback(
        harness,
        slug,
        &idp_code(&nonce, subject, username, groups),
        &state,
    )
    .await
}

async fn exchange(harness: &Harness, redirect: &str) -> (StatusCode, Value) {
    let code = query_param(redirect, "code");
    let response = raw(
        harness,
        Method::POST,
        "/api/auth/oidc/exchange",
        &[],
        Some(json!({"code": code})),
    )
    .await;
    (response.status, response.body)
}

async fn role_of(harness: &Harness, user_id: Uuid) -> Uuid {
    entity::UserOrganization::find()
        .filter(entity::user_organization::Column::UserId.eq(user_id))
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap()
        .role_id
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn provider_crud_validates_and_persists() {
    let harness = full_harness().await;
    let role = create_role(&harness.db, harness.org_id).await;
    let identifier = slug();
    let created = create_provider(&harness, provider_body(&identifier, Some(role))).await;
    assert_eq!(created["slug"], identifier);
    assert_eq!(created["user_count"], 0);
    assert!(created["redirect_uri"]
        .as_str()
        .unwrap()
        .ends_with(&format!("/api/auth/oidc/{identifier}/callback")));
    assert!(created.get("client_secret").is_none());

    let (status, _) = harness
        .post("/api/identity-providers", provider_body(&identifier, None))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let invalid = [
        json!({"slug": "Bad Slug"}),
        json!({"display_name": " "}),
        json!({"client_id": ""}),
        json!({"issuer_url": "http://public.example.com"}),
        json!({"client_secret": ""}),
        json!({"default_role_id": Uuid::new_v4()}),
    ];
    for patch in invalid {
        let mut body = provider_body(&slug(), None);
        body.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let (status, _) = harness.post("/api/identity-providers", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{patch}");
    }

    let (status, listed) = harness.get("/api/identity-providers").await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["slug"] == identifier));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn provider_update_and_delete() {
    let harness = full_harness().await;
    let created = create_provider(&harness, provider_body(&slug(), None)).await;
    let uri = format!(
        "/api/identity-providers/{}",
        created["id"].as_str().unwrap()
    );

    let update = json!({
        "display_name": "Renamed", "issuer_url": idp_issuer(), "client_id": "other",
        "client_secret": "new-secret", "enabled": false, "auto_provision": false,
    });
    let (status, updated) = harness.admin(Method::PUT, &uri, Some(update.clone())).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["display_name"], "Renamed");
    assert_eq!(updated["enabled"], false);

    let invalid = json!({"display_name": "", "issuer_url": idp_issuer(), "client_id": "x"});
    assert_eq!(
        harness.admin(Method::PUT, &uri, Some(invalid)).await.0,
        StatusCode::BAD_REQUEST
    );
    let missing = format!("/api/identity-providers/{}", Uuid::new_v4());
    assert_eq!(
        harness.admin(Method::PUT, &missing, Some(update)).await.0,
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn group_mappings_are_replaced_atomically() {
    let harness = full_harness().await;
    let created = create_provider(&harness, provider_body(&slug(), None)).await;
    let uri = format!(
        "/api/identity-providers/{}/group-mappings",
        created["id"].as_str().unwrap()
    );
    let role = create_role(&harness.db, harness.org_id).await;

    let mappings = json!([
        {"external_group": "ops", "role_id": role, "priority": 1},
        {"external_group": "dev", "role_id": role, "priority": 5},
    ]);
    assert_eq!(
        harness.admin(Method::PUT, &uri, Some(mappings)).await.0,
        StatusCode::NO_CONTENT
    );
    let (_, listed) = harness.get(&uri).await;
    assert_eq!(listed[0]["external_group"], "dev");
    assert_eq!(listed.as_array().unwrap().len(), 2);

    let foreign = json!([{"external_group": "x", "role_id": Uuid::new_v4()}]);
    assert_eq!(
        harness.admin(Method::PUT, &uri, Some(foreign)).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        harness.admin(Method::PUT, &uri, Some(json!([]))).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(harness.get(&uri).await.1, json!([]));

    let missing = format!("/api/identity-providers/{}/group-mappings", Uuid::new_v4());
    assert_eq!(
        harness
            .admin(Method::PUT, &missing, Some(json!([])))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn connection_test_discovers_or_fails() {
    let harness = full_harness().await;
    let created = create_provider(&harness, provider_body(&slug(), None)).await;
    let uri = format!(
        "/api/identity-providers/{}/test",
        created["id"].as_str().unwrap()
    );
    let (status, endpoints) = harness.post(&uri, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(endpoints["issuer"], idp_issuer());

    let mut body = provider_body(&slug(), None);
    body["issuer_url"] = json!(idp_mismatch_issuer());
    let mismatched = create_provider(&harness, body).await;
    let uri = format!(
        "/api/identity-providers/{}/test",
        mismatched["id"].as_str().unwrap()
    );
    assert_eq!(
        harness.post(&uri, json!({})).await.0,
        StatusCode::BAD_GATEWAY
    );
    let missing = format!("/api/identity-providers/{}/test", Uuid::new_v4());
    assert_eq!(
        harness.post(&missing, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn single_sign_on_provisions_and_reuses_users() {
    let harness = full_harness().await;
    let role = create_role(&harness.db, harness.org_id).await;
    let identifier = slug();
    let created = create_provider(&harness, provider_body(&identifier, Some(role))).await;
    let (_, public) = harness
        .send(Method::GET, "/api/auth/providers", None, None)
        .await;
    assert!(public
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["slug"] == identifier));

    let username = format!("sso-{}", Uuid::new_v4().simple());
    let subject = format!("sub-{}", Uuid::new_v4().simple());
    let redirect = login(&harness, &identifier, &subject, &username, "").await;
    assert!(
        redirect.starts_with("http://gw.test/login/callback?code="),
        "{redirect}"
    );
    let (status, session) = exchange(&harness, &redirect).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["username"], username);
    let user_id: Uuid = session["user_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(role_of(&harness, user_id).await, role);
    assert_eq!(
        exchange(&harness, &redirect).await.0,
        StatusCode::UNAUTHORIZED
    );

    let again = login(&harness, &identifier, &subject, &username, "").await;
    let (_, second) = exchange(&harness, &again).await;
    assert_eq!(second["user_id"], session["user_id"]);

    let uri = format!(
        "/api/identity-providers/{}",
        created["id"].as_str().unwrap()
    );
    let (status, _) = harness
        .admin(Method::DELETE, &format!("{uri}?delete_users=true"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let remaining = entity::User::find_by_id(user_id)
        .one(&harness.db)
        .await
        .unwrap();
    assert!(remaining.is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn group_mappings_select_the_role_with_highest_priority() {
    let harness = full_harness().await;
    let fallback = create_role(&harness.db, harness.org_id).await;
    let mapped = create_role(&harness.db, harness.org_id).await;
    let identifier = slug();
    let created = create_provider(&harness, provider_body(&identifier, Some(fallback))).await;
    let mappings = json!([{"external_group": "admins", "role_id": mapped, "priority": 10}]);
    let uri = format!(
        "/api/identity-providers/{}/group-mappings",
        created["id"].as_str().unwrap()
    );
    assert_eq!(
        harness.admin(Method::PUT, &uri, Some(mappings)).await.0,
        StatusCode::NO_CONTENT
    );

    let username = format!("sso-{}", Uuid::new_v4().simple());
    let redirect = login(&harness, &identifier, &username, &username, "staff,admins").await;
    let (_, session) = exchange(&harness, &redirect).await;
    let user_id: Uuid = session["user_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(role_of(&harness, user_id).await, mapped);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn single_sign_on_rejections_redirect_with_a_code() {
    let harness = full_harness().await;
    let identifier = slug();
    create_provider(&harness, provider_body(&identifier, None)).await;

    let denied = login(&harness, &identifier, "s1", "someone", "").await;
    assert!(denied.ends_with("sso_error=no_role"), "{denied}");

    let unknown = format!("/api/auth/oidc/{}/start", slug());
    let response = raw(&harness, Method::GET, &unknown, &ORIGIN_HEADERS, None).await;
    assert!(location(&response).ends_with("sso_error=unknown_provider"));

    let bad_state = callback(&harness, &identifier, "x~y~z~w~", "missing-state").await;
    assert!(bad_state.ends_with("sso_error=invalid_state"));

    let uri = format!("/api/auth/oidc/{identifier}/callback?error=access_denied");
    let response = raw(&harness, Method::GET, &uri, &ORIGIN_HEADERS, None).await;
    assert!(location(&response).ends_with("sso_error=provider_denied"));
    let uri = format!("/api/auth/oidc/{identifier}/callback");
    let response = raw(&harness, Method::GET, &uri, &ORIGIN_HEADERS, None).await;
    assert!(location(&response).ends_with("sso_error=invalid_state"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn single_sign_on_rejects_bad_tokens_and_unprovisioned_users() {
    let harness = full_harness().await;
    let role = create_role(&harness.db, harness.org_id).await;
    let identifier = slug();
    create_provider(&harness, provider_body(&identifier, Some(role))).await;

    let (state, _) = begin(&harness, &identifier).await;
    let mismatch = callback(
        &harness,
        &identifier,
        &idp_code("wrong-nonce", "s", "u", ""),
        &state,
    )
    .await;
    assert!(mismatch.ends_with("sso_error=nonce_mismatch"), "{mismatch}");

    let (state, _) = begin(&harness, &identifier).await;
    let refused = callback(&harness, &identifier, "deny~s~u~e~", &state).await;
    assert!(
        refused.ends_with("sso_error=provider_unavailable"),
        "{refused}"
    );

    let closed = slug();
    let mut body = provider_body(&closed, Some(role));
    body["auto_provision"] = json!(false);
    create_provider(&harness, body).await;
    let outcome = login(&harness, &closed, "stranger", "stranger", "").await;
    assert!(outcome.ends_with("sso_error=not_provisioned"), "{outcome}");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn disabled_providers_are_hidden_and_usernames_do_not_collide() {
    let harness = full_harness().await;
    let role = create_role(&harness.db, harness.org_id).await;
    let hidden = slug();
    let mut body = provider_body(&hidden, Some(role));
    body["enabled"] = json!(false);
    create_provider(&harness, body).await;
    let (_, public) = harness
        .send(Method::GET, "/api/auth/providers", None, None)
        .await;
    assert!(!public
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["slug"] == hidden));

    let identifier = slug();
    create_provider(&harness, provider_body(&identifier, Some(role))).await;
    let taken = harness.admin_name.clone();
    let redirect = login(&harness, &identifier, "collide", &taken, "").await;
    let (_, session) = exchange(&harness, &redirect).await;
    assert_eq!(session["username"], format!("{taken}@{identifier}"));

    let invalid = raw(
        &harness,
        Method::POST,
        "/api/auth/oidc/exchange",
        &[],
        Some(json!({"code": "nope"})),
    )
    .await;
    assert_eq!(invalid.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn provider_admin_routes_require_permission() {
    let harness = full_harness().await;
    let limited = harness.limited_token().await;
    let id = Uuid::new_v4();
    let cases = [
        (Method::GET, "/api/identity-providers".to_string()),
        (Method::POST, "/api/identity-providers".to_string()),
        (Method::PUT, format!("/api/identity-providers/{id}")),
        (Method::DELETE, format!("/api/identity-providers/{id}")),
        (Method::POST, format!("/api/identity-providers/{id}/test")),
        (
            Method::GET,
            format!("/api/identity-providers/{id}/group-mappings"),
        ),
        (
            Method::PUT,
            format!("/api/identity-providers/{id}/group-mappings"),
        ),
    ];
    for (method, uri) in cases {
        let body = Some(json!({}));
        assert_eq!(
            harness
                .send(method.clone(), &uri, None, body.clone())
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
        assert_eq!(
            harness.send(method, &uri, Some(&limited), body).await.0,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
}
