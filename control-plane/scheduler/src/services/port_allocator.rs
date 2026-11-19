use crate::models::workload::PortMapping;

const DEFAULT_NODE_PORT_RANGE_START: u16 = 30000;
const DEFAULT_NODE_PORT_RANGE_END: u16 = 32767;

fn node_port_range() -> (u16, u16) {
    let start = std::env::var("NODE_PORT_RANGE_START")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_NODE_PORT_RANGE_START);
    let end = std::env::var("NODE_PORT_RANGE_END")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_NODE_PORT_RANGE_END);
    (start, end)
}

pub fn ensure_requested_in_range(ports: &[PortMapping]) -> Result<(), String> {
    let (start, end) = node_port_range();
    match ports
        .iter()
        .filter_map(|mapping| mapping.node_port)
        .find(|port| !(start..=end).contains(port))
    {
        Some(port) => Err(format!(
            "node port {port} is outside the allowed range {start}-{end}"
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(node_port: Option<u16>) -> PortMapping {
        PortMapping {
            container_port: 80,
            protocol: None,
            rg_port: None,
            node_port,
        }
    }

    #[test]
    fn explicit_port_outside_range_is_rejected() {
        for port in [22, 7443, 29999, 32768] {
            let error = ensure_requested_in_range(&[mapping(Some(port))]).unwrap_err();
            assert!(error.contains(&port.to_string()));
        }
    }

    #[test]
    fn explicit_port_inside_range_is_accepted() {
        assert!(ensure_requested_in_range(&[mapping(Some(30000)), mapping(None)]).is_ok());
    }
}
