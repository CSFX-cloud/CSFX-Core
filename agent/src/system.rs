use sysinfo::{Disks, Networks, System};

pub struct SystemInfo {
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
}

pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub uptime_seconds: u64,
}

fn parse_os_release_field(content: &str, field: &str) -> Option<String> {
    content
        .lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.splitn(2, '=').nth(1))
        .map(|v| v.trim_matches('"').to_string())
}

fn detect_os() -> (String, String) {
    if let Ok(os_type) = std::env::var("CSFX_OS_TYPE") {
        let os_version = std::env::var("CSFX_OS_VERSION")
            .unwrap_or_else(|_| System::os_version().unwrap_or_else(|| "unknown".to_string()));
        return (os_type.to_lowercase(), os_version);
    }

    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        let id = parse_os_release_field(&content, "ID");
        let version = parse_os_release_field(&content, "VERSION_ID")
            .or_else(|| parse_os_release_field(&content, "BUILD_ID"));

        if let Some(os_type) = id {
            let os_version = version.unwrap_or_else(|| {
                System::os_version().unwrap_or_else(|| "unknown".to_string())
            });
            return (os_type.to_lowercase(), os_version);
        }
    }

    (
        System::name().unwrap_or_else(|| "linux".to_string()).to_lowercase(),
        System::os_version().unwrap_or_else(|| "unknown".to_string()),
    )
}

pub fn collect_info() -> SystemInfo {
    let (os_type, os_version) = detect_os();
    SystemInfo {
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        os_type,
        os_version,
        architecture: std::env::consts::ARCH.to_string(),
    }
}

pub fn collect_metrics() -> SystemMetrics {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage_percent = sys.global_cpu_usage();
    let cpu_cores = sys.cpus().len() as u32;
    let memory_total_bytes = sys.total_memory();
    let memory_used_bytes = sys.used_memory();

    let disks = Disks::new_with_refreshed_list();
    let (disk_total_bytes, disk_used_bytes) = disks.iter().fold((0u64, 0u64), |(total, used), d| {
        (total + d.total_space(), used + (d.total_space() - d.available_space()))
    });

    let networks = Networks::new_with_refreshed_list();
    let (network_rx_bytes, network_tx_bytes) =
        networks.iter().fold((0u64, 0u64), |(rx, tx), (_, data)| {
            (rx + data.total_received(), tx + data.total_transmitted())
        });

    SystemMetrics {
        cpu_usage_percent,
        cpu_cores,
        memory_total_bytes,
        memory_used_bytes,
        disk_total_bytes,
        disk_used_bytes,
        network_rx_bytes,
        network_tx_bytes,
        uptime_seconds: System::uptime(),
    }
}
