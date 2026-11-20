use serde::{Deserialize, Serialize};

pub const ARCHITECTURES: [&str; 2] = ["x86_64", "aarch64"];
pub const NIC_MODELS: [&str; 2] = ["virtio", "e1000e"];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmConfig {
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(default)]
    pub nic_model: Option<String>,
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
        allowed(&self.nic_model, &NIC_MODELS, "nic model")
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
        };
        assert!(config.validate().is_ok());
        assert!(VmConfig::default().validate().is_ok());
    }

    #[test]
    fn unknown_values_are_rejected() {
        let architecture = VmConfig {
            architecture: Some("riscv64".to_string()),
            nic_model: None,
        };
        assert!(architecture
            .validate()
            .unwrap_err()
            .contains("architecture"));
        let nic = VmConfig {
            architecture: None,
            nic_model: Some("rtl8139".to_string()),
        };
        assert!(nic.validate().unwrap_err().contains("nic model"));
    }
}
