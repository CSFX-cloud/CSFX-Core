use anyhow::{Context, Result};
use tracing::warn;

use super::{short_id, QemuRuntime, TAP_DEVICE_PREFIX};
use crate::rg_dhcp::mac_address_for_workload;
use crate::spec::{ExtraNic, WorkloadSpec};
use crate::vm_host::{self, process_uid};

const EXTRA_TAP_PREFIX: &str = "qtx";

pub(super) struct GuestNic {
    pub tap_device: String,
    pub mac_address: String,
    pub ip: Option<String>,
}

pub(super) fn tap_device_name(workload_id: &str, index: usize) -> String {
    match index {
        0 => format!("{TAP_DEVICE_PREFIX}{}", short_id(workload_id)),
        _ => format!("{EXTRA_TAP_PREFIX}{index}{}", short_id(workload_id)),
    }
}

fn mac_address_for_nic(workload_id: &str, index: usize) -> String {
    match index {
        0 => mac_address_for_workload(workload_id),
        _ => mac_address_for_workload(&format!("{workload_id}/{index}")),
    }
}

fn guest_nics(
    workload_id: &str,
    primary_ip: Option<String>,
    extra_nics: &[ExtraNic],
) -> Vec<GuestNic> {
    std::iter::once(primary_ip)
        .chain(extra_nics.iter().map(|nic| nic.ip.clone()))
        .enumerate()
        .map(|(index, ip)| GuestNic {
            tap_device: tap_device_name(workload_id, index),
            mac_address: mac_address_for_nic(workload_id, index),
            ip,
        })
        .collect()
}

pub(super) fn tap_devices(workload_id: &str, extra_nic_count: usize) -> Vec<String> {
    (0..=extra_nic_count)
        .map(|index| tap_device_name(workload_id, index))
        .collect()
}

pub(super) async fn remove_taps(workload_id: &str, tap_devices: &[String]) {
    for tap_device in tap_devices {
        if let Err(e) = vm_host::remove_tap(tap_device).await {
            warn!(workload_id = %workload_id, tap_device = %tap_device, error = %e, "Failed to remove tap device");
        }
    }
}

async fn create_taps(workload_id: &str, nics: &[GuestNic], bridge: Option<&str>) -> Result<()> {
    let mut created = Vec::with_capacity(nics.len());
    for nic in nics {
        if let Err(e) = vm_host::create_tap(&nic.tap_device, bridge, process_uid()).await {
            remove_taps(workload_id, &created).await;
            return Err(e);
        }
        created.push(nic.tap_device.clone());
    }
    Ok(())
}

impl QemuRuntime {
    pub(super) async fn attach_networks(
        &self,
        spec: &WorkloadSpec,
        extra_nics: &[ExtraNic],
        bridge_iface: Option<&str>,
    ) -> Result<Vec<GuestNic>> {
        let group = spec.resource_group_id.as_deref();
        let cidr = spec.resource_group_cidr.as_deref();
        let primary_ip = match (group, cidr) {
            (Some(group), Some(cidr)) => Some(
                crate::rg_ipam::allocate(group, cidr, &spec.workload_id, spec.guest_ip.as_deref())
                    .await?,
            ),
            _ => None,
        };
        let nics = guest_nics(&spec.workload_id, primary_ip, extra_nics);

        if let (Some(group), Some(cidr), Some(bridge)) = (group, cidr, bridge_iface) {
            self.reserve_addresses(spec, group, cidr, bridge, &nics)
                .await?;
        }
        create_taps(&spec.workload_id, &nics, bridge_iface).await?;
        Ok(nics)
    }

    async fn reserve_addresses(
        &self,
        spec: &WorkloadSpec,
        group: &str,
        cidr: &str,
        bridge: &str,
        nics: &[GuestNic],
    ) -> Result<()> {
        for (index, nic) in nics.iter().enumerate() {
            let ip = nic
                .ip
                .as_deref()
                .with_context(|| format!("network interface {index} has no assigned address"))?;
            if index > 0 {
                crate::rg_ipam::claim(group, &spec.workload_id, ip).await?;
            }
            self.dhcp_supervisor
                .add_reservation(group, cidr, bridge, &spec.workload_id, &nic.mac_address, ip)
                .await
                .context("Failed to register resource group dhcp reservation")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_names_are_distinct_and_fit_linux_interface_limits() {
        let id = "12345678-1234-1234-1234-123456789abc";
        let names = tap_devices(id, 3);
        assert_eq!(names[0], "qtap12345678123");
        assert_eq!(names[1], "qtx112345678123");
        assert!(names.iter().all(|name| name.len() == 15));
        assert_eq!(
            names.iter().collect::<std::collections::HashSet<_>>().len(),
            4
        );
    }

    #[test]
    fn nics_get_their_own_mac_and_keep_the_primary_one() {
        let extras = [ExtraNic {
            ip: Some("10.0.0.9".to_string()),
        }];
        let nics = guest_nics("w", Some("10.0.0.3".to_string()), &extras);
        assert_eq!(nics[0].mac_address, mac_address_for_workload("w"));
        assert_ne!(nics[1].mac_address, nics[0].mac_address);
        assert_eq!(nics[1].ip.as_deref(), Some("10.0.0.9"));
    }
}
