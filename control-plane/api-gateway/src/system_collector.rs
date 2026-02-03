use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSystemMetrics {
    pub timestamp: DateTime<Utc>,

    // CPU
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub cpu_usage_percent: f32,

    // Memory
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_usage_percent: f32,

    // Disk
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_usage_percent: f32,

    // Network
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,

    // System
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub hostname: String,
    pub uptime_seconds: u64,
}

pub struct LocalSystemCollector {
    system: Arc<Mutex<System>>,
    networks: Arc<Mutex<Networks>>,
    disks: Arc<Mutex<Disks>>,
}

impl LocalSystemCollector {
    pub fn new() -> Self {
        Self {
            system: Arc::new(Mutex::new(System::new_all())),
            networks: Arc::new(Mutex::new(Networks::new_with_refreshed_list())),
            disks: Arc::new(Mutex::new(Disks::new_with_refreshed_list())),
        }
    }

    pub fn collect(&self) -> LocalSystemMetrics {
        // Refresh all data
        let mut system = self.system.lock().unwrap();
        system.refresh_all();

        let mut networks = self.networks.lock().unwrap();
        networks.refresh();

        let mut disks = self.disks.lock().unwrap();
        disks.refresh();

        // CPU info
        let cpu_model = system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let cpu_cores = system.physical_core_count().unwrap_or(0) as u32;
        let cpu_threads = system.cpus().len() as u32;

        let cpu_usage_percent = if !system.cpus().is_empty() {
            let total: f32 = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum();
            total / system.cpus().len() as f32
        } else {
            0.0
        };

        // Memory
        let memory_total_bytes = system.total_memory();
        let memory_used_bytes = system.used_memory();
        let memory_usage_percent = if memory_total_bytes > 0 {
            (memory_used_bytes as f32 / memory_total_bytes as f32) * 100.0
        } else {
            0.0
        };

        // Disk
        let (disk_total_bytes, disk_used_bytes) =
            disks.iter().fold((0u64, 0u64), |(total, used), disk| {
                (
                    total + disk.total_space(),
                    used + (disk.total_space() - disk.available_space()),
                )
            });

        let disk_usage_percent = if disk_total_bytes > 0 {
            (disk_used_bytes as f32 / disk_total_bytes as f32) * 100.0
        } else {
            0.0
        };

        // Network
        let (network_rx_bytes, network_tx_bytes) =
            networks
                .iter()
                .fold((0u64, 0u64), |(rx, tx), (_name, network)| {
                    (
                        rx + network.total_received(),
                        tx + network.total_transmitted(),
                    )
                });

        // System info
        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let uptime_seconds = System::uptime();

        LocalSystemMetrics {
            timestamp: Utc::now(),
            cpu_model,
            cpu_cores,
            cpu_threads,
            cpu_usage_percent,
            memory_total_bytes,
            memory_used_bytes,
            memory_usage_percent,
            disk_total_bytes,
            disk_used_bytes,
            disk_usage_percent,
            network_rx_bytes,
            network_tx_bytes,
            os_name,
            os_version,
            kernel_version,
            hostname,
            uptime_seconds,
        }
    }
}

impl Default for LocalSystemCollector {
    fn default() -> Self {
        Self::new()
    }
}
