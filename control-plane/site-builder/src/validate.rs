const MAX_URL_LENGTH: usize = 2048;
const MAX_REF_LENGTH: usize = 255;
const MAX_NAME_LENGTH: usize = 63;
const MAX_ENV_NAME_LENGTH: usize = 128;
const MAX_COMMAND_LENGTH: usize = 1024;
const RESERVED_ENV_PREFIX: &str = "CSFX_";

pub fn repo_url(value: &str) -> Result<(), &'static str> {
    let remainder = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or("repository url must start with https:// or http://")?;
    if value.len() > MAX_URL_LENGTH || remainder.is_empty() || remainder.starts_with('/') {
        return Err("repository url is invalid");
    }
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("repository url must not contain whitespace");
    }
    Ok(())
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
    fn repo_urls_must_be_http_without_whitespace() {
        assert!(repo_url("https://github.com/org/repo.git").is_ok());
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
