use std::collections::HashSet;
use std::net::Ipv4Addr;

const FIRST_GUEST_OFFSET: u32 = 3;
const MIN_PREFIX: u32 = 8;
const MAX_PREFIX: u32 = 29;

fn subnet(cidr: &str) -> Option<(u32, u32)> {
    let (address, prefix) = cidr.split_once('/')?;
    let address: Ipv4Addr = address.parse().ok()?;
    let prefix: u32 = prefix
        .parse()
        .ok()
        .filter(|prefix| (MIN_PREFIX..=MAX_PREFIX).contains(prefix))?;

    let host_count = 1u32 << (32 - prefix);
    Some((u32::from(address) & !(host_count - 1), host_count))
}

pub fn next_free(cidr: &str, used: &HashSet<String>) -> Option<String> {
    let (network, host_count) = subnet(cidr)?;

    (FIRST_GUEST_OFFSET..host_count - 1)
        .map(|offset| Ipv4Addr::from(network + offset).to_string())
        .find(|candidate| !used.contains(candidate))
}

fn is_guest_address(cidr: &str, ip: &str) -> bool {
    let (Some((network, host_count)), Ok(ip)) = (subnet(cidr), ip.parse::<Ipv4Addr>()) else {
        return false;
    };
    let offset = u32::from(ip).wrapping_sub(network);
    (FIRST_GUEST_OFFSET..host_count - 1).contains(&offset)
}

pub fn assign(
    cidr: &str,
    used: &HashSet<String>,
    requested: &[Option<String>],
) -> Result<Vec<String>, String> {
    let mut taken = used.clone();
    for ip in requested.iter().flatten() {
        if !is_guest_address(cidr, ip) {
            return Err(format!("{ip} is not an assignable address in {cidr}"));
        }
        if !taken.insert(ip.clone()) {
            return Err(format!("{ip} is already in use"));
        }
    }

    requested
        .iter()
        .map(|request| match request {
            Some(ip) => Ok(ip.clone()),
            None => {
                let ip =
                    next_free(cidr, &taken).ok_or_else(|| format!("no free guest ip in {cidr}"))?;
                taken.insert(ip.clone());
                Ok(ip)
            }
        })
        .collect()
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

    #[test]
    fn assign_fills_automatic_entries_around_requested_addresses() {
        let requested = [None, Some("10.10.0.3".to_string()), None];
        assert_eq!(
            assign("10.10.0.0/24", &used(&[]), &requested),
            Ok(vec![
                "10.10.0.4".to_string(),
                "10.10.0.3".to_string(),
                "10.10.0.5".to_string()
            ])
        );
    }

    #[test]
    fn assign_rejects_used_duplicate_and_out_of_range_requests() {
        let at = |ip: &str| vec![Some(ip.to_string())];
        let taken = used(&["10.10.0.7"]);
        assert!(assign("10.10.0.0/24", &taken, &at("10.10.0.7")).is_err());
        assert!(assign("10.10.0.0/24", &used(&[]), &at("10.10.1.7")).is_err());
        assert!(assign("10.10.0.0/24", &used(&[]), &at("10.10.0.1")).is_err());
        assert!(assign("10.10.0.0/24", &used(&[]), &at("10.10.0.255")).is_err());
        let twice = [Some("10.10.0.9".to_string()), Some("10.10.0.9".to_string())];
        assert!(assign("10.10.0.0/24", &used(&[]), &twice).is_err());
    }
}
