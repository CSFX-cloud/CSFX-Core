use tokio::process::Command;
use tracing::warn;

pub async fn resolve(port: u16) -> Option<String> {
    if let Ok(configured) = std::env::var("CSFX_WG_ENDPOINT") {
        return Some(configured);
    }
    let address = match std::env::var("CSFX_ADVERTISE_INTERFACE") {
        Ok(interface) if !interface.is_empty() => interface_address(&interface).await,
        _ => default_route_address(),
    };
    address.map(|address| format!("{address}:{port}"))
}

fn default_route_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

async fn interface_address(interface: &str) -> Option<String> {
    let output = Command::new("ip")
        .args(["-o", "-4", "addr", "show", "dev", interface])
        .output()
        .await
        .map_err(|e| warn!(interface = %interface, error = %e, "failed to run ip for advertise interface"))
        .ok()?;
    let address = parse_ipv4(&String::from_utf8_lossy(&output.stdout));
    if address.is_none() {
        warn!(interface = %interface, "advertise interface has no ipv4 address");
    }
    address
}

fn parse_ipv4(listing: &str) -> Option<String> {
    let mut tokens = listing.split_whitespace();
    tokens.find(|token| *token == "inet")?;
    let cidr = tokens.next()?;
    Some(cidr.split('/').next()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_ipv4;

    #[test]
    fn address_is_taken_from_the_first_inet_entry() {
        let listing = "3: enp0s2    inet 192.168.64.2/24 metric 100 brd 192.168.64.255 scope global dynamic enp0s2\\       valid_lft 3000sec";
        assert_eq!(parse_ipv4(listing), Some("192.168.64.2".to_string()));
    }

    #[test]
    fn missing_address_yields_none() {
        assert_eq!(parse_ipv4(""), None);
        assert_eq!(
            parse_ipv4("3: enp0s2    link/ether 52:54:00:12:34:56"),
            None
        );
    }
}
