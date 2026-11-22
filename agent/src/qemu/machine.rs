use anyhow::{bail, Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const FIRMWARE_CODE_ENV: &str = "CSFX_QEMU_FIRMWARE_CODE_PATH";
const FIRMWARE_VARS_ENV: &str = "CSFX_QEMU_FIRMWARE_VARS_PATH";
const FIRMWARE_VARS_FILE: &str = "efi-vars.fd";
const VARS_FILE_MODE: u32 = 0o600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Architecture {
    X86_64,
    Aarch64,
}

fn owned<const N: usize>(args: [&str; N]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

pub const DEFAULT_NIC_MODEL: &str = "virtio";

pub fn nic_device_args(nic_model: Option<&str>, mac_address: &str, index: usize) -> Vec<String> {
    let device = match nic_model.unwrap_or(DEFAULT_NIC_MODEL) {
        "e1000e" => "e1000e",
        _ => "virtio-net-pci",
    };
    vec![
        "-device".to_string(),
        format!("{device},netdev=net{index},mac={mac_address}"),
    ]
}

pub fn data_disk_args(index: usize, device_path: &Path) -> Vec<String> {
    vec![
        "-drive".to_string(),
        format!(
            "file={},format=raw,if=none,id=data{index},cache=writeback",
            device_path.display()
        ),
        "-device".to_string(),
        format!("virtio-blk-pci,drive=data{index}"),
    ]
}

impl Architecture {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "x86_64" => Some(Self::X86_64),
            "aarch64" => Some(Self::Aarch64),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }

    pub const HOST: Self = if cfg!(target_arch = "aarch64") {
        Self::Aarch64
    } else {
        Self::X86_64
    };

    pub fn default_qemu_bin_path(self) -> &'static str {
        match self {
            Self::X86_64 => "/usr/bin/qemu-system-x86_64",
            Self::Aarch64 => "/usr/bin/qemu-system-aarch64",
        }
    }

    pub fn machine_args(self) -> Vec<String> {
        match self {
            Self::X86_64 => owned(["-enable-kvm", "-machine", "q35", "-cpu", "host"]),
            Self::Aarch64 => owned([
                "-enable-kvm",
                "-machine",
                "virt,gic-version=max",
                "-cpu",
                "host",
            ]),
        }
    }

    pub fn display_args(self) -> Vec<String> {
        match self {
            Self::X86_64 => owned(["-vga", "std", "-usb", "-device", "usb-tablet"]),
            Self::Aarch64 => owned([
                "-device",
                "virtio-gpu-pci",
                "-device",
                "qemu-xhci",
                "-device",
                "usb-kbd",
                "-device",
                "usb-tablet",
            ]),
        }
    }

    pub fn iso_drive_args(self, iso_path: &Path) -> Vec<String> {
        let iso = iso_path.display();
        match self {
            Self::X86_64 => vec![
                "-drive".to_string(),
                format!("file={iso},media=cdrom,format=raw,readonly=on,if=ide"),
                "-boot".to_string(),
                "order=cd".to_string(),
            ],
            Self::Aarch64 => vec![
                "-drive".to_string(),
                format!("file={iso},media=cdrom,format=raw,readonly=on,if=none,id=install-iso"),
                "-device".to_string(),
                "virtio-scsi-pci,id=install-scsi".to_string(),
                "-device".to_string(),
                "scsi-cd,drive=install-iso,bootindex=0".to_string(),
            ],
        }
    }
}

fn firmware_path(variable: &str) -> Result<String> {
    std::env::var(variable)
        .with_context(|| format!("{variable} must point to the uefi firmware for aarch64 vms"))
}

pub async fn firmware_args(architecture: Architecture, vm_dir: &Path) -> Result<Vec<String>> {
    if architecture != Architecture::Aarch64 {
        return Ok(Vec::new());
    }
    let code_path = firmware_path(FIRMWARE_CODE_ENV)?;
    let vars_template = firmware_path(FIRMWARE_VARS_ENV)?;
    let vars_path = vm_dir.join(FIRMWARE_VARS_FILE);
    if tokio::fs::metadata(&vars_path).await.is_err() {
        tokio::fs::copy(&vars_template, &vars_path)
            .await
            .with_context(|| format!("Failed to copy uefi variable store {vars_template}"))?;
        tokio::fs::set_permissions(&vars_path, std::fs::Permissions::from_mode(VARS_FILE_MODE))
            .await
            .context("Failed to make the uefi variable store writable")?;
    }
    if tokio::fs::metadata(&code_path).await.is_err() {
        bail!("uefi firmware {code_path} does not exist");
    }
    Ok(vec![
        "-drive".to_string(),
        format!("if=pflash,format=raw,unit=0,readonly=on,file={code_path}"),
        "-drive".to_string(),
        format!("if=pflash,format=raw,unit=1,file={}", vars_path.display()),
    ])
}

#[cfg(test)]
mod tests;
