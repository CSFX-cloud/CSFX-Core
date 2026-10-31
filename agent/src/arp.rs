use anyhow::{bail, Context, Result};
use std::net::Ipv4Addr;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

const ETHERTYPE_ARP: u16 = 0x0806;
const ARP_REPLY: u16 = 2;
const HARDWARE_ETHERNET: u16 = 1;
const PROTOCOL_IPV4: u16 = 0x0800;
const BROADCAST: [u8; 6] = [0xff; 6];
const FRAME_LENGTH: usize = 42;

pub fn announce(iface: &str, address: Ipv4Addr) -> Result<()> {
    let mac = read_mac(iface)?;
    let index = read_index(iface)?;
    let frame = gratuitous_frame(mac, address);

    let socket = open_arp_socket()?;
    let mut target: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    target.sll_family = libc::AF_PACKET as u16;
    target.sll_ifindex = index;
    target.sll_halen = 6;
    target.sll_addr[..6].copy_from_slice(&BROADCAST);

    let sent = unsafe {
        libc::sendto(
            socket.as_raw_fd(),
            frame.as_ptr().cast(),
            frame.len(),
            0,
            (&target as *const libc::sockaddr_ll).cast(),
            std::mem::size_of::<libc::sockaddr_ll>() as u32,
        )
    };
    if sent < 0 {
        return Err(std::io::Error::last_os_error()).context("failed to send gratuitous arp");
    }
    Ok(())
}

fn open_arp_socket() -> Result<OwnedFd> {
    let fd = unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW,
            i32::from(ETHERTYPE_ARP.to_be()),
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("failed to open arp socket");
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn read_mac(iface: &str) -> Result<[u8; 6]> {
    let raw = std::fs::read_to_string(format!("/sys/class/net/{}/address", iface))
        .with_context(|| format!("failed to read mac iface={}", iface))?;
    let octets: Vec<u8> = raw
        .trim()
        .split(':')
        .filter_map(|octet| u8::from_str_radix(octet, 16).ok())
        .collect();
    match octets.try_into() {
        Ok(mac) => Ok(mac),
        Err(_) => bail!("unexpected mac format iface={}", iface),
    }
}

fn read_index(iface: &str) -> Result<i32> {
    std::fs::read_to_string(format!("/sys/class/net/{}/ifindex", iface))
        .with_context(|| format!("failed to read ifindex iface={}", iface))?
        .trim()
        .parse()
        .with_context(|| format!("unexpected ifindex iface={}", iface))
}

fn gratuitous_frame(mac: [u8; 6], address: Ipv4Addr) -> [u8; FRAME_LENGTH] {
    let mut frame = [0u8; FRAME_LENGTH];
    frame[0..6].copy_from_slice(&BROADCAST);
    frame[6..12].copy_from_slice(&mac);
    frame[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());
    frame[14..16].copy_from_slice(&HARDWARE_ETHERNET.to_be_bytes());
    frame[16..18].copy_from_slice(&PROTOCOL_IPV4.to_be_bytes());
    frame[18] = 6;
    frame[19] = 4;
    frame[20..22].copy_from_slice(&ARP_REPLY.to_be_bytes());
    frame[22..28].copy_from_slice(&mac);
    frame[28..32].copy_from_slice(&address.octets());
    frame[32..38].copy_from_slice(&BROADCAST);
    frame[38..42].copy_from_slice(&address.octets());
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_announces_the_address_with_the_bridge_mac() {
        let mac = [0x02, 0x11, 0x22, 0x33, 0x44, 0x55];
        let frame = gratuitous_frame(mac, Ipv4Addr::new(10, 20, 0, 4));

        assert_eq!(&frame[0..6], &BROADCAST);
        assert_eq!(&frame[6..12], &mac);
        assert_eq!(&frame[12..14], &[0x08, 0x06]);
        assert_eq!(&frame[20..22], &[0x00, 0x02]);
        assert_eq!(&frame[22..28], &mac);
        assert_eq!(&frame[28..32], &[10, 20, 0, 4]);
        assert_eq!(&frame[38..42], &[10, 20, 0, 4]);
    }
}
