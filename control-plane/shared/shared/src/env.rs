use std::env;
use std::str::FromStr;

pub fn parsed<T: FromStr>(name: &str, default: T) -> Result<T, String> {
    match env::var(name) {
        Ok(raw) => raw
            .parse()
            .map_err(|_| format!("{name} has invalid value {raw}")),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::parsed;

    #[test]
    fn unset_variable_uses_default() {
        assert_eq!(parsed("CSFX_ENV_TEST_UNSET", 7u32).unwrap(), 7);
    }

    #[test]
    fn invalid_value_is_rejected() {
        unsafe { std::env::set_var("CSFX_ENV_TEST_INVALID", "many") };
        assert!(parsed("CSFX_ENV_TEST_INVALID", 7u32).is_err());
    }
}
