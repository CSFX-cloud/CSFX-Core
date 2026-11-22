use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

pub const ARCHITECTURES: [&str; 2] = ["x86_64", "aarch64"];
pub const NIC_MODELS: [&str; 2] = ["virtio", "e1000e"];
pub const MAX_EXTRA_NICS: usize = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraNic {
    #[serde(default)]
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmConfig {
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(default)]
    pub nic_model: Option<String>,
    #[serde(default)]
    pub guest_ip: Option<String>,
    #[serde(default)]
    pub extra_nics: Vec<ExtraNic>,
}

fn allowed(value: &Option<String>, options: &[&str], label: &str) -> Result<(), String> {
    match value {
        Some(value) if !options.contains(&value.as_str()) => {
            Err(format!("{label} must be one of {}", options.join(", ")))
        }
        _ => Ok(()),
    }
}

impl VmConfig {
    pub fn validate(&self) -> Result<(), String> {
        allowed(&self.architecture, &ARCHITECTURES, "architecture")?;
        allowed(&self.nic_model, &NIC_MODELS, "nic model")?;
        if self.extra_nics.len() > MAX_EXTRA_NICS {
            return Err(format!(
                "a vm supports at most {MAX_EXTRA_NICS} additional addresses"
            ));
        }
        self.requested_ips()
            .iter()
            .flatten()
            .try_for_each(|ip| match ip.parse::<Ipv4Addr>() {
                Ok(_) => Ok(()),
                Err(_) => Err(format!("{ip} is not a valid ipv4 address")),
            })
    }

    pub fn requested_ips(&self) -> Vec<Option<String>> {
        std::iter::once(self.guest_ip.clone())
            .chain(self.extra_nics.iter().map(|nic| nic.ip.clone()))
            .collect()
    }

    pub fn extra_ips(&self) -> impl Iterator<Item = &String> {
        self.extra_nics.iter().filter_map(|nic| nic.ip.as_ref())
    }

    pub fn with_assigned_ips(&self, ips: &[String]) -> Self {
        Self {
            guest_ip: None,
            extra_nics: ips
                .iter()
                .skip(1)
                .map(|ip| ExtraNic {
                    ip: Some(ip.clone()),
                })
                .collect(),
            ..self.clone()
        }
    }

    pub fn without_addresses(&self) -> Self {
        Self {
            guest_ip: None,
            extra_nics: vec![ExtraNic::default(); self.extra_nics.len()],
            ..self.clone()
        }
    }

    pub fn requests_addresses(&self) -> bool {
        self.guest_ip.is_some() || !self.extra_nics.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_are_accepted() {
        let config = VmConfig {
            architecture: Some("aarch64".to_string()),
            nic_model: Some("e1000e".to_string()),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
        assert!(VmConfig::default().validate().is_ok());
    }

    #[test]
    fn unknown_values_are_rejected() {
        let architecture = VmConfig {
            architecture: Some("riscv64".to_string()),
            ..Default::default()
        };
        assert!(architecture
            .validate()
            .unwrap_err()
            .contains("architecture"));
        let nic = VmConfig {
            nic_model: Some("rtl8139".to_string()),
            ..Default::default()
        };
        assert!(nic.validate().unwrap_err().contains("nic model"));
    }

    #[test]
    fn addresses_are_validated() {
        let bad_ip = VmConfig {
            guest_ip: Some("10.0.0".to_string()),
            ..Default::default()
        };
        assert!(bad_ip.validate().unwrap_err().contains("ipv4"));
        let too_many = VmConfig {
            extra_nics: vec![ExtraNic::default(); MAX_EXTRA_NICS + 1],
            ..Default::default()
        };
        assert!(too_many.validate().unwrap_err().contains("at most"));
    }

    #[test]
    fn assigned_ips_replace_requests_and_can_be_stripped() {
        let config = VmConfig {
            guest_ip: Some("10.0.0.9".to_string()),
            extra_nics: vec![ExtraNic::default(); 2],
            ..Default::default()
        };
        let ips = ["10.0.0.9", "10.0.0.3", "10.0.0.4"].map(String::from);
        let assigned = config.with_assigned_ips(&ips);
        assert_eq!(assigned.guest_ip, None);
        assert_eq!(assigned.extra_ips().collect::<Vec<_>>(), [&ips[1], &ips[2]]);
        let stripped = assigned.without_addresses();
        assert_eq!(stripped.extra_nics.len(), 2);
        assert_eq!(stripped.extra_ips().count(), 0);
    }
}
