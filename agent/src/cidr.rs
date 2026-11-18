use anyhow::{Context, Result};
use std::net::Ipv4Addr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cidr {
    address: u32,
    prefix: u8,
}

impl Cidr {
    pub const fn new(address: u32, prefix: u8) -> Self {
        Self { address, prefix }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (address, prefix) = text.split_once('/')?;
        let address: Ipv4Addr = address.parse().ok()?;
        let prefix: u8 = prefix.parse().ok()?;
        (prefix <= 32).then(|| Self::new(u32::from(address), prefix))
    }

    pub fn parse_network(text: &str) -> Result<Self> {
        let (address, prefix) = text.split_once('/').context("cidr has no prefix")?;
        let address: Ipv4Addr = address.parse().context("cidr address is invalid")?;
        let prefix: u8 = prefix.parse().context("cidr prefix is invalid")?;
        anyhow::ensure!(prefix <= 32, "cidr prefix is out of range");
        let cidr = Self::new(u32::from(address), prefix);
        anyhow::ensure!(
            cidr.address & cidr.mask() == cidr.address,
            "cidr has host bits set"
        );
        Ok(cidr)
    }

    pub fn prefix(self) -> u8 {
        self.prefix
    }

    pub fn mask(self) -> u32 {
        u32::MAX
            .checked_shl(32 - u32::from(self.prefix))
            .unwrap_or(0)
    }

    pub fn host_count(self) -> u32 {
        1u32.checked_shl(32 - u32::from(self.prefix))
            .unwrap_or(u32::MAX)
    }

    pub fn nth_host(self, offset: u32) -> Option<Ipv4Addr> {
        self.address.checked_add(offset).map(Ipv4Addr::from)
    }

    pub fn overlaps(self, other: Self) -> bool {
        let mask = Self::new(0, self.prefix.min(other.prefix)).mask();
        self.address & mask == other.address & mask
    }

    pub fn encloses(self, other: Self) -> bool {
        other.prefix >= self.prefix && self.overlaps(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_parsing_keeps_host_bits() {
        let cidr = Cidr::parse("10.1.0.255/16").unwrap();
        assert_eq!(cidr.nth_host(1), Some(Ipv4Addr::new(10, 1, 1, 0)));
        assert!(Cidr::parse("10.1.0.0").is_none());
        assert!(Cidr::parse("10.1.0/24").is_none());
        assert!(Cidr::parse("10.0.0.0/33").is_none());
        assert!(Cidr::parse("10.0.0.0/x").is_none());
    }

    #[test]
    fn network_parsing_rejects_host_bits() {
        assert!(Cidr::parse_network("10.101.0.0/24").is_ok());
        assert!(Cidr::parse_network("10.1.2.3/24").is_err());
        assert!(Cidr::parse_network("10.0.0.0").is_err());
        assert!(Cidr::parse_network("10.0.0.0/33").is_err());
    }

    #[test]
    fn host_counts_follow_the_prefix() {
        assert_eq!(Cidr::parse("10.0.0.0/24").unwrap().host_count(), 256);
        assert_eq!(Cidr::parse("10.0.0.0/32").unwrap().host_count(), 1);
        assert_eq!(Cidr::parse("0.0.0.0/0").unwrap().host_count(), u32::MAX);
    }

    #[test]
    fn overlap_and_enclosure_compare_the_shorter_prefix() {
        let wide = Cidr::new(0x0A000000, 8);
        let narrow = Cidr::new(0x0A650000, 24);
        assert!(wide.overlaps(narrow));
        assert!(narrow.overlaps(wide));
        assert!(wide.encloses(narrow));
        assert!(!narrow.encloses(wide));
        assert!(!wide.overlaps(Cidr::new(0xC0A80000, 16)));
    }
}
