use serde::Serialize;
use std::path::Path;
use sysinfo::Disks;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct SmartInfo {
    pub health: String,
    pub power_on_hours: Option<u64>,
    pub temperature_celsius: Option<f32>,
    pub reallocated_sectors: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub device: String,
    pub model: Option<String>,
    pub media_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub mount_point: Option<String>,
    pub smart: Option<SmartInfo>,
}

pub async fn collect_disks() -> Vec<DiskInfo> {
    if std::env::var("CSFX_FAKE_DISKS").is_ok() {
        return fake_disks();
    }

    let block_devices = list_block_devices();
    let mounts = Disks::new_with_refreshed_list();

    let mut disks = Vec::with_capacity(block_devices.len());
    for device in block_devices {
        let media_type = detect_media_type(&device);
        let model = read_sys_string(&device, "device/model");
        let mount = mounts
            .iter()
            .find(|d| d.name().to_string_lossy().contains(&device));

        let total_bytes = mount.map(|d| d.total_space()).unwrap_or(0);
        let used_bytes = mount
            .map(|d| d.total_space() - d.available_space())
            .unwrap_or(0);
        let mount_point = mount.map(|d| d.mount_point().to_string_lossy().to_string());

        disks.push(DiskInfo {
            smart: collect_smart(&device).await,
            device,
            model,
            media_type,
            total_bytes,
            used_bytes,
            mount_point,
        });
    }

    disks
}

fn fake_disks() -> Vec<DiskInfo> {
    vec![
        DiskInfo {
            device: "nvme0n1".to_string(),
            model: Some("Samsung SSD 990 PRO 2TB".to_string()),
            media_type: "nvme".to_string(),
            total_bytes: 2_000_398_934_016,
            used_bytes: 812_000_000_000,
            mount_point: Some("/".to_string()),
            smart: Some(SmartInfo {
                health: "healthy".to_string(),
                power_on_hours: Some(3120),
                temperature_celsius: Some(41.0),
                reallocated_sectors: Some(0),
            }),
        },
        DiskInfo {
            device: "sda".to_string(),
            model: Some("Seagate IronWolf 8TB".to_string()),
            media_type: "hdd".to_string(),
            total_bytes: 8_001_563_222_016,
            used_bytes: 5_200_000_000_000,
            mount_point: Some("/mnt/csfx-volumes".to_string()),
            smart: Some(SmartInfo {
                health: "healthy".to_string(),
                power_on_hours: Some(18544),
                temperature_celsius: Some(34.0),
                reallocated_sectors: Some(0),
            }),
        },
        DiskInfo {
            device: "sdb".to_string(),
            model: Some("Seagate IronWolf 8TB".to_string()),
            media_type: "hdd".to_string(),
            total_bytes: 8_001_563_222_016,
            used_bytes: 7_600_000_000_000,
            mount_point: None,
            smart: Some(SmartInfo {
                health: "failing".to_string(),
                power_on_hours: Some(31022),
                temperature_celsius: Some(52.0),
                reallocated_sectors: Some(12),
            }),
        },
    ]
}

fn list_block_devices() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return Vec::new();
    };

    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| !name.starts_with("loop") && !name.starts_with("ram"))
        .collect()
}

fn read_sys_string(device: &str, relative_path: &str) -> Option<String> {
    let path = format!("/sys/block/{device}/{relative_path}");
    std::fs::read_to_string(path)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn detect_media_type(device: &str) -> String {
    if device.starts_with("nvme") {
        return "nvme".to_string();
    }
    match read_sys_string(device, "queue/rotational").as_deref() {
        Some("0") => "ssd".to_string(),
        Some("1") => "hdd".to_string(),
        _ => "unknown".to_string(),
    }
}

async fn collect_smart(device: &str) -> Option<SmartInfo> {
    if !Path::new("/dev").join(device).exists() {
        return None;
    }

    let output = Command::new("smartctl")
        .args(["-j", "-a", &format!("/dev/{device}")])
        .output()
        .await
        .ok()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    parse_smart_json(&json)
}

fn parse_smart_json(json: &serde_json::Value) -> Option<SmartInfo> {
    let passed = json
        .get("smart_status")
        .and_then(|s| s.get("passed"))
        .and_then(|p| p.as_bool());

    let health = match passed {
        Some(true) => "healthy",
        Some(false) => "failing",
        None => return None,
    }
    .to_string();

    let power_on_hours = json
        .get("power_on_time")
        .and_then(|p| p.get("hours"))
        .and_then(|h| h.as_u64());

    let temperature_celsius = json
        .get("temperature")
        .and_then(|t| t.get("current"))
        .and_then(|c| c.as_f64())
        .map(|c| c as f32);

    let reallocated_sectors = json
        .get("ata_smart_attributes")
        .and_then(|a| a.get("table"))
        .and_then(|t| t.as_array())
        .and_then(|attrs| {
            attrs
                .iter()
                .find(|a| a.get("id").and_then(|i| i.as_u64()) == Some(5))
        })
        .and_then(|a| a.get("raw"))
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_u64());

    Some(SmartInfo {
        health,
        power_on_hours,
        temperature_celsius,
        reallocated_sectors,
    })
}
