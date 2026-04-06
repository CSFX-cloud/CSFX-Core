use anyhow::Result;

use crate::config::Config;

pub const AVAILABLE_FLAKE_REV_KEY: &str = "/csf/config/available_flake_rev";
pub const DESIRED_FLAKE_REV_KEY: &str = "/csf/config/desired_flake_rev";
pub const BUILD_STATUS_KEY: &str = "/csf/config/cp_build_status";
pub const RESULT_KEY: &str = "/csf/config/last_build_result";
pub const PAUSED_KEY: &str = "/csf/config/update_paused";

pub struct Client {
    inner: etcd_client::Client,
}

impl Client {
    pub async fn connect(cfg: &Config) -> Result<Self> {
        let endpoints: Vec<&str> = cfg.etcd_endpoints.iter().map(|s| s.as_str()).collect();
        let inner = etcd_client::Client::connect(endpoints, None).await?;
        Ok(Self { inner })
    }

    pub async fn get(&mut self, key: &str) -> Result<Option<String>> {
        let resp = self.inner.get(key, None).await?;
        Ok(resp
            .kvs()
            .first()
            .and_then(|kv| std::str::from_utf8(kv.value()).ok())
            .map(|s| s.to_string()))
    }

    pub async fn put(&mut self, key: &str, value: &str) -> Result<()> {
        self.inner.put(key, value.as_bytes(), None).await?;
        Ok(())
    }
}
