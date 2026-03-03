use sysinfo::System;

pub struct SystemInfo {
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
}

pub fn collect_info() -> SystemInfo {
    SystemInfo {
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        os_type: System::name().unwrap_or_else(|| "linux".to_string()).to_lowercase(),
        os_version: System::os_version().unwrap_or_else(|| "unknown".to_string()),
        architecture: std::env::consts::ARCH.to_string(),
    }
}
