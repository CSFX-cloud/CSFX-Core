use etcd_client::Client;

pub const ENDPOINTS_VAR: &str = "ETCD_ENDPOINTS";
pub const DEFAULT_ENDPOINTS: &str = "http://localhost:2379";
pub const WORKER_MANIFEST_PREFIX: &str = "/csfx/config/worker_release_manifest/";
pub const WORKER_MANIFEST_SIGNATURE_PREFIX: &str = "/csfx/config/worker_release_manifest_sig/";

pub fn parse_endpoints(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn endpoints_from_env() -> Vec<String> {
    let raw = std::env::var(ENDPOINTS_VAR).unwrap_or_else(|_| DEFAULT_ENDPOINTS.to_string());
    let endpoints = parse_endpoints(&raw);
    if endpoints.is_empty() {
        return parse_endpoints(DEFAULT_ENDPOINTS);
    }
    endpoints
}

pub async fn connect(endpoints: &[String]) -> Result<Client, etcd_client::Error> {
    Client::connect(endpoints, None).await
}

pub async fn connect_from_env() -> Result<Client, etcd_client::Error> {
    connect(&endpoints_from_env()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_lists_are_split_and_trimmed() {
        assert_eq!(
            parse_endpoints("http://a:2379, http://b:2379,,"),
            vec!["http://a:2379", "http://b:2379"]
        );
    }

    #[test]
    fn blank_lists_yield_no_endpoints() {
        assert!(parse_endpoints(" , ").is_empty());
    }
}
