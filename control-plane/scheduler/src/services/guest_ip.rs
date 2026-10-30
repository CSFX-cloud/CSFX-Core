use std::collections::HashSet;
use std::net::Ipv4Addr;

const FIRST_GUEST_OFFSET: u32 = 3;
const MIN_PREFIX: u32 = 8;
const MAX_PREFIX: u32 = 29;

pub fn next_free(cidr: &str, used: &HashSet<String>) -> Option<String> {
    let (address, prefix) = cidr.split_once('/')?;
    let address: Ipv4Addr = address.parse().ok()?;
    let prefix: u32 = prefix
        .parse()
        .ok()
        .filter(|prefix| (MIN_PREFIX..=MAX_PREFIX).contains(prefix))?;

    let host_count = 1u32 << (32 - prefix);
    let network = u32::from(address) & !(host_count - 1);

    (FIRST_GUEST_OFFSET..host_count - 1)
        .map(|offset| Ipv4Addr::from(network + offset).to_string())
        .find(|candidate| !used.contains(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(ips: &[&str]) -> HashSet<String> {
        ips.iter().map(|ip| ip.to_string()).collect()
    }

    #[test]
    fn starts_after_the_gateway_and_dns_address() {
        assert_eq!(
            next_free("10.10.0.0/24", &used(&[])),
            Some("10.10.0.3".to_string())
        );
    }

    #[test]
    fn skips_used_addresses_and_reuses_gaps() {
        assert_eq!(
            next_free("10.10.0.0/24", &used(&["10.10.0.3", "10.10.0.5"])),
            Some("10.10.0.4".to_string())
        );
    }

    #[test]
    fn returns_none_when_the_subnet_is_exhausted() {
        assert_eq!(
            next_free(
                "10.10.0.0/29",
                &used(&["10.10.0.3", "10.10.0.4", "10.10.0.5", "10.10.0.6"])
            ),
            None
        );
    }

    #[test]
    fn rejects_malformed_or_unsupported_cidrs() {
        assert_eq!(next_free("not-a-cidr", &used(&[])), None);
        assert_eq!(next_free("10.10.0.0/31", &used(&[])), None);
        assert_eq!(next_free("10.10.0.0/4", &used(&[])), None);
    }
}
