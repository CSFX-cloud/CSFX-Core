use anyhow::{bail, Context, Result};
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Identifier {
    Numeric(u64),
    Text(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Version {
    core: Vec<u64>,
    pre_release: Option<Vec<Identifier>>,
}

impl Version {
    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim_start_matches('v');
        let (core, pre_release) = match raw.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (raw, None),
        };
        let core = core
            .split('.')
            .map(|part| {
                part.parse::<u64>()
                    .with_context(|| format!("invalid version {raw}"))
            })
            .collect::<Result<Vec<_>>>()?;
        if core.len() != 3 {
            bail!("version must have three numeric components: {raw}");
        }
        let pre_release = pre_release.map(parse_identifiers);
        Ok(Self { core, pre_release })
    }
}

fn parse_identifiers(raw: &str) -> Vec<Identifier> {
    raw.split('.')
        .map(|part| match part.parse::<u64>() {
            Ok(number) => Identifier::Numeric(number),
            Err(_) => Identifier::Text(part.to_string()),
        })
        .collect()
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (&self.pre_release, &other.pre_release) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(left), Some(right)) => left.cmp(right),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::Version;

    fn parse(raw: &str) -> Version {
        Version::parse(raw).unwrap()
    }

    #[test]
    fn build_numbers_order_numerically() {
        assert!(parse("0.2.2-alpha.1034") > parse("0.2.2-alpha.1033"));
        assert!(parse("0.2.2-alpha.10") > parse("0.2.2-alpha.2"));
    }

    #[test]
    fn release_outranks_its_pre_releases() {
        assert!(parse("0.3.0") > parse("0.3.0-rc.1"));
        assert!(parse("0.3.0-rc.1") > parse("0.3.0-beta.9"));
        assert!(parse("0.3.0-beta.1") > parse("0.3.0-alpha.9"));
    }

    #[test]
    fn core_components_dominate() {
        assert!(parse("v0.3.0") > parse("0.2.9"));
        assert!(parse("1.0.0-alpha.1") > parse("0.9.9"));
    }

    #[test]
    fn malformed_versions_are_rejected() {
        assert!(Version::parse("latest").is_err());
        assert!(Version::parse("1.2").is_err());
    }
}
