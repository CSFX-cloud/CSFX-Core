use axum::http::{header, HeaderMap};
use cookie::{time::Duration, Cookie, SameSite};

use super::sha256_base64url;

pub const STATE_COOKIE: &str = "csfx_oidc_state";
pub const EXCHANGE_COOKIE: &str = "csfx_oidc_exchange";

const COOKIE_PATH: &str = "/api/auth/oidc";
const STATE_COOKIE_SECONDS: i64 = 600;
const EXCHANGE_COOKIE_SECONDS: i64 = 60;

fn base(name: &'static str, value: String, origin: &str) -> cookie::CookieBuilder<'static> {
    Cookie::build((name, value))
        .path(COOKIE_PATH)
        .http_only(true)
        .secure(origin.starts_with("https://"))
        .same_site(SameSite::Lax)
}

pub fn bind_state(origin: &str, state: &str) -> String {
    base(STATE_COOKIE, sha256_base64url(state), origin)
        .max_age(Duration::seconds(STATE_COOKIE_SECONDS))
        .build()
        .to_string()
}

pub fn bind_exchange_code(origin: &str, code: &str) -> String {
    base(EXCHANGE_COOKIE, sha256_base64url(code), origin)
        .max_age(Duration::seconds(EXCHANGE_COOKIE_SECONDS))
        .build()
        .to_string()
}

pub fn clear(origin: &str, name: &'static str) -> String {
    base(name, String::new(), origin)
        .max_age(Duration::ZERO)
        .build()
        .to_string()
}

fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| {
            Cookie::split_parse(value.to_string())
                .flatten()
                .collect::<Vec<_>>()
        })
        .find(|cookie| cookie.name() == name)
        .map(|cookie| cookie.value().to_string())
}

fn is_bound(headers: &HeaderMap, name: &str, secret: &str) -> bool {
    read(headers, name).is_some_and(|stored| stored == sha256_base64url(secret))
}

pub fn state_is_bound(headers: &HeaderMap, state: &str) -> bool {
    is_bound(headers, STATE_COOKIE, state)
}

pub fn exchange_code_is_bound(headers: &HeaderMap, code: &str) -> bool {
    is_bound(headers, EXCHANGE_COOKIE, code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers_with(cookie_line: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_str(cookie_line).unwrap());
        headers
    }

    fn pair_of(set_cookie: &str) -> String {
        set_cookie.split(';').next().unwrap().to_string()
    }

    #[test]
    fn state_binding_only_matches_the_browser_that_started_the_login() {
        let cookie = pair_of(&bind_state("https://gw.test", "state-1"));
        let owner = headers_with(&format!("other=1; {cookie}"));
        assert!(state_is_bound(&owner, "state-1"));
        assert!(!state_is_bound(&owner, "state-2"));
        assert!(!state_is_bound(&HeaderMap::new(), "state-1"));
        assert!(!state_is_bound(
            &headers_with("csfx_oidc_state=forged"),
            "state-1"
        ));
    }

    #[test]
    fn exchange_binding_is_separate_from_the_state_binding() {
        let cookie = pair_of(&bind_exchange_code("https://gw.test", "code-1"));
        let owner = headers_with(&cookie);
        assert!(exchange_code_is_bound(&owner, "code-1"));
        assert!(!exchange_code_is_bound(&owner, "code-2"));
        assert!(!state_is_bound(&owner, "code-1"));
    }

    #[test]
    fn cookies_are_http_only_lax_and_secure_only_over_https() {
        let secure = bind_state("https://gw.test", "s");
        assert!(secure.contains("HttpOnly"));
        assert!(secure.contains("SameSite=Lax"));
        assert!(secure.contains("Secure"));
        assert!(secure.contains("Path=/api/auth/oidc"));
        assert!(!bind_state("http://gw.test", "s").contains("Secure"));
        assert!(clear("https://gw.test", STATE_COOKIE).contains("Max-Age=0"));
    }
}
