use anyhow::Result;
use etcd_client::ConnectOptions;

use crate::config::Config;

pub const DESIRED_VERSION_KEY: &str = "/csf/config/desired_cp_version";
pub const RESULT_KEY: &str = "/csf/config/last_update_result";

pub struct Client {
    inner: etcd_client::Client,
}

impl Client {
    pub async fn connect(cfg: &Config) -> Result<Self> {
        let endpoints: Vec<&str> = cfg.etcd_endpoints.iter().map(|s| s.as_str()).collect();
        let opts = ConnectOptions::new()
            .with_user(cfg.etcd_username.clone(), cfg.etcd_password.clone());

        let inner = etcd_client::Client::connect(endpoints, Some(opts)).await?;
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
