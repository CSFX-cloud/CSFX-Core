use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const MAX_URL_LENGTH: usize = 2048;
const MAX_REF_LENGTH: usize = 255;
const MAX_NAME_LENGTH: usize = 63;
const MAX_ENV_NAME_LENGTH: usize = 128;
const MAX_COMMAND_LENGTH: usize = 1024;
const RESERVED_ENV_PREFIX: &str = "CSFX_";
pub const REPOSITORY_NOT_ALLOWED: &str = "repository url is not allowed";

pub fn repo_url(value: &str) -> Result<(), &'static str> {
    if !value.starts_with("https://") {
        return Err("repository url must start with https://");
    }
    repo_url_syntax(value)?;
    let url = reqwest::Url::parse(value).map_err(|_| "repository url is invalid")?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("repository url must not contain credentials");
    }
    let host = url.host_str().ok_or(REPOSITORY_NOT_ALLOWED)?;
    let literal = host.trim_start_matches('[').trim_end_matches(']');
    let allowed = match literal.parse::<IpAddr>() {
        Ok(address) => is_public_address(address),
        Err(_) => !is_local_name(host),
    };
    allowed.then_some(()).ok_or(REPOSITORY_NOT_ALLOWED)
}

pub fn repo_url_syntax(value: &str) -> Result<(), &'static str> {
    let remainder = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or("repository url must start with https://")?;
    if value.len() > MAX_URL_LENGTH || remainder.is_empty() || remainder.starts_with('/') {
        return Err("repository url is invalid");
    }
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("repository url must not contain whitespace");
    }
    Ok(())
}

fn is_local_name(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name == "localhost" || name.ends_with(".localhost")
}

pub fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_v4(address),
        IpAddr::V6(address) => match address.to_ipv4_mapped() {
            Some(mapped) => is_public_v4(mapped),
            None => is_public_v6(address),
        },
    }
}

fn is_public_v4(address: Ipv4Addr) -> bool {
    let [first, second, third, _] = address.octets();
    let carrier_grade_nat = first == 100 && (64..=127).contains(&second);
    let protocol_assignments = first == 192 && second == 0 && third == 0;
    let benchmarking = first == 198 && (18..=19).contains(&second);
    let reserved = first >= 240;
    !(first == 0
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_documentation()
        || address.is_multicast()
        || carrier_grade_nat
        || protocol_assignments
        || benchmarking
        || reserved)
}

fn is_public_v6(address: Ipv6Addr) -> bool {
    let first = address.segments()[0];
    let unique_local = first & 0xfe00 == 0xfc00;
    let link_local = first & 0xffc0 == 0xfe80;
    let documentation = first == 0x2001 && address.segments()[1] == 0x0db8;
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || unique_local
        || link_local
        || documentation)
}

pub fn branch(value: &str) -> Result<(), &'static str> {
    let first = value.chars().next().ok_or("branch must not be empty")?;
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-');
    if value.len() > MAX_REF_LENGTH || !first.is_ascii_alphanumeric() {
        return Err("branch is invalid");
    }
    if !value.chars().all(allowed) || value.contains("..") {
        return Err("branch is invalid");
    }
    Ok(())
}

pub fn root_dir(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Ok(());
    }
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-');
    if value.len() > MAX_REF_LENGTH || value.starts_with('/') || !value.chars().all(allowed) {
        return Err("root directory is invalid");
    }
    if value.split('/').any(|part| part == "..") {
        return Err("root directory must stay inside the repository");
    }
    Ok(())
}

pub fn site_name(value: &str) -> Result<(), &'static str> {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    let valid = !value.is_empty()
        && value.len() <= MAX_NAME_LENGTH
        && value.chars().all(allowed)
        && !value.starts_with('-')
        && !value.ends_with('-');
    valid
        .then_some(())
        .ok_or("name must be a lowercase dns label")
}

pub fn env_name(value: &str) -> Result<(), &'static str> {
    let mut chars = value.chars();
    let first = chars.next().ok_or("variable name must not be empty")?;
    let valid = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && value.len() <= MAX_ENV_NAME_LENGTH;
    if !valid {
        return Err("variable name is invalid");
    }
    if value.starts_with(RESERVED_ENV_PREFIX) {
        return Err("variable name uses a reserved prefix");
    }
    Ok(())
}

pub fn command(value: &str) -> Result<(), &'static str> {
    if value.len() > MAX_COMMAND_LENGTH || value.contains('\0') {
        return Err("command is invalid");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_urls_must_be_public_https_without_credentials() {
        assert!(repo_url("https://github.com/org/repo.git").is_ok());
        assert!(repo_url("https://8.8.8.8/org/repo.git").is_ok());
        for bad in [
            "http://github.com/org/repo",
            "http://127.0.0.1:2379/x",
            "https://127.0.0.1/x",
            "https://localhost/x",
            "https://api.localhost/x",
            "https://10.0.0.5/x",
            "https://172.16.0.1/x",
            "https://192.168.1.1/x",
            "https://169.254.169.254/latest",
            "https://100.64.0.1/x",
            "https://0.0.0.0/x",
            "https://[::1]/x",
            "https://[fd00::1]/x",
            "https://[fe80::1]/x",
            "https://[::ffff:10.0.0.1]/x",
            "https://user:pass@github.com/org/repo",
            "https://token@github.com/org/repo",
        ] {
            assert!(repo_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn address_classification_matches_the_reserved_ranges() {
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.0.1",
            "169.254.1.1",
            "::1",
            "fc00::1",
        ] {
            assert!(!is_public_address(private.parse().unwrap()), "{private}");
        }
        for public in ["1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(is_public_address(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn repo_urls_must_not_contain_whitespace_or_odd_schemes() {
        assert!(repo_url_syntax("http://127.0.0.1:1/x.git").is_ok());
        for bad in [
            "",
            "git@github.com:a/b",
            "file:///etc",
            "https://",
            "https:///x",
            "https://a b",
            "ext::sh -c id",
        ] {
            assert!(repo_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn branches_cannot_inject_options_or_traverse() {
        assert!(branch("feature/new-checkout").is_ok());
        for bad in ["", "-x", "--upload-pack=sh", "a..b", "a b", "a;b"] {
            assert!(branch(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn root_dirs_stay_inside_the_repository() {
        assert!(root_dir("").is_ok());
        assert!(root_dir("apps/web").is_ok());
        for bad in ["/etc", "../x", "a/../../b", "a b"] {
            assert!(root_dir(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names_and_variables_follow_their_grammar() {
        assert!(site_name("shop-web").is_ok());
        for bad in ["", "-a", "a-", "Shop", "a_b"] {
            assert!(site_name(bad).is_err(), "{bad}");
        }
        assert!(env_name("PUBLIC_API_URL").is_ok());
        for bad in ["", "1A", "A-B", "CSFX_TOKEN"] {
            assert!(env_name(bad).is_err(), "{bad}");
        }
    }
}
