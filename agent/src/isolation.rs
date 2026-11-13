use crate::wireguard::{self, Peer};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tracing::warn;

const PROBE_PORT: u16 = 9;
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn isolated_from_peers() -> bool {
    let peers = match wireguard::load_peer_cache() {
        Ok(peers) => peers,
        Err(error) => {
            warn!(error = %error, "peer cache unreadable, assuming not isolated");
            return false;
        }
    };
    let targets = probe_targets(&peers);
    if targets.is_empty() {
        return false;
    }
    !any_reachable(targets).await
}

fn probe_targets(peers: &[Peer]) -> Vec<String> {
    peers
        .iter()
        .filter(|peer| peer.endpoint.is_some())
        .filter_map(|peer| peer.allowed_ips.split('/').next())
        .map(|address| format!("{address}:{PROBE_PORT}"))
        .collect()
}

async fn any_reachable(targets: Vec<String>) -> bool {
    let mut probes = JoinSet::new();
    for target in targets {
        probes.spawn(async move { peer_answers(&target).await });
    }
    while let Some(outcome) = probes.join_next().await {
        if outcome.unwrap_or(false) {
            return true;
        }
    }
    false
}

async fn peer_answers(target: &str) -> bool {
    match tokio::time::timeout(PROBE_TIMEOUT, TcpStream::connect(target)).await {
        Ok(Ok(_)) => true,
        Ok(Err(error)) => error.kind() == std::io::ErrorKind::ConnectionRefused,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(endpoint: Option<&str>, allowed_ips: &str) -> Peer {
        Peer {
            public_key: "key".to_string(),
            endpoint: endpoint.map(str::to_string),
            allowed_ips: allowed_ips.to_string(),
        }
    }

    #[test]
    fn only_mesh_peers_with_an_endpoint_are_probed() {
        let peers = vec![
            peer(Some("192.168.105.5:51820"), "10.100.0.3/32"),
            peer(None, "10.200.0.9/32"),
        ];
        assert_eq!(probe_targets(&peers), vec!["10.100.0.3:9".to_string()]);
    }

    #[tokio::test]
    async fn listening_and_refusing_peers_count_as_reachable() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let open = listener.local_addr().unwrap().to_string();
        assert!(peer_answers(&open).await);
        drop(listener);
        assert!(peer_answers(&open).await);
    }

    #[tokio::test]
    async fn no_reachable_target_means_isolated() {
        assert!(!any_reachable(Vec::new()).await);
    }
}
