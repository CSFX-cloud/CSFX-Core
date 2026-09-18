use serde::Serialize;
use sysinfo::{Disks, Networks, System};

pub struct SystemInfo {
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
}

#[derive(Serialize)]
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

pub struct LiveMetricsCollector {
    sys: System,
    simulator: Option<LoadSimulator>,
}

impl LiveMetricsCollector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_cpu_usage();
        let simulator = std::env::var("CSFX_SIMULATE_LOAD")
            .ok()
            .filter(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .map(|_| LoadSimulator::new(sys.total_memory(), sys.cpus().len() as u32));
        Self { sys, simulator }
    }

    pub fn sample(&mut self) -> SystemMetrics {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        if let Some(simulator) = &mut self.simulator {
            return simulator.sample();
        }

        let disks = Disks::new_with_refreshed_list();
        let (disk_total_bytes, disk_used_bytes) =
            disks.iter().fold((0u64, 0u64), |(total, used), d| {
                (
                    total + d.total_space(),
                    used + (d.total_space() - d.available_space()),
                )
            });

        let cpu_usage_percent = self.sys.global_cpu_usage();
        let cpu_cores = self.sys.cpus().len() as u32;
        let memory_total_bytes = self.sys.total_memory();
        let memory_used_bytes = self.sys.used_memory();

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
}

struct LoadSimulator {
    memory_total_bytes: u64,
    disk_total_bytes: u64,
    cpu_cores: u32,
    cpu_usage_percent: f32,
    memory_usage_percent: f32,
    disk_usage_percent: f32,
    network_rx_bytes: u64,
    network_tx_bytes: u64,
    seed: u64,
}

impl LoadSimulator {
    fn new(memory_total_bytes: u64, cpu_cores: u32) -> Self {
        Self {
            memory_total_bytes,
            disk_total_bytes: 512 * 1_073_741_824,
            cpu_cores: cpu_cores.max(1),
            cpu_usage_percent: 25.0,
            memory_usage_percent: 35.0,
            disk_usage_percent: 42.0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            seed: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1),
        }
    }

    fn next_random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed % 10_000) as f32 / 10_000.0
    }

    fn walk(&mut self, value: f32, max_step: f32, min: f32, max: f32) -> f32 {
        let step = (self.next_random() - 0.5) * 2.0 * max_step;
        (value + step).clamp(min, max)
    }

    fn sample(&mut self) -> SystemMetrics {
        self.cpu_usage_percent = self.walk(self.cpu_usage_percent, 8.0, 3.0, 95.0);
        self.memory_usage_percent = self.walk(self.memory_usage_percent, 3.0, 10.0, 90.0);
        self.disk_usage_percent = self.walk(self.disk_usage_percent, 0.2, 5.0, 85.0);

        let rx_rate_bytes = (self.walk(2.0, 1.5, 0.05, 12.0) * 1_048_576.0) as u64;
        let tx_rate_bytes = (self.walk(0.8, 0.6, 0.02, 5.0) * 1_048_576.0) as u64;
        self.network_rx_bytes += rx_rate_bytes;
        self.network_tx_bytes += tx_rate_bytes;

        let memory_used_bytes =
            ((self.memory_usage_percent / 100.0) as f64 * self.memory_total_bytes as f64) as u64;
        let disk_used_bytes =
            ((self.disk_usage_percent / 100.0) as f64 * self.disk_total_bytes as f64) as u64;

        SystemMetrics {
            cpu_usage_percent: self.cpu_usage_percent,
            cpu_cores: self.cpu_cores,
            memory_total_bytes: self.memory_total_bytes,
            memory_used_bytes,
            disk_total_bytes: self.disk_total_bytes,
            disk_used_bytes,
            network_rx_bytes: self.network_rx_bytes,
            network_tx_bytes: self.network_tx_bytes,
            uptime_seconds: System::uptime(),
        }
    }
}

fn parse_os_release_field(content: &str, field: &str) -> Option<String> {
    content
        .lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.split_once('=').map(|x| x.1))
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
            let os_version = version
                .unwrap_or_else(|| System::os_version().unwrap_or_else(|| "unknown".to_string()));
            return (os_type.to_lowercase(), os_version);
        }
    }

    (
        System::name()
            .unwrap_or_else(|| "linux".to_string())
            .to_lowercase(),
        System::os_version().unwrap_or_else(|| "unknown".to_string()),
    )
}

pub fn is_kvm_capable() -> bool {
    std::path::Path::new("/dev/kvm").exists()
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
    let (disk_total_bytes, disk_used_bytes) =
        disks.iter().fold((0u64, 0u64), |(total, used), d| {
            (
                total + d.total_space(),
                used + (d.total_space() - d.available_space()),
            )
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
