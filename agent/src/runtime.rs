use anyhow::Result;
use axum::body::Bytes;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::spec::WorkloadSpec;

pub type LogStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

pub struct ExecSession {
    pub input: Pin<Box<dyn AsyncWrite + Send>>,
    pub output: Pin<Box<dyn AsyncRead + Send>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageEntry {
    pub name: String,
    pub bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolumeUsage {
    pub mount_path: String,
    pub used_bytes: i64,
    pub total_bytes: i64,
    #[serde(default)]
    pub entries: Vec<UsageEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct ContainerStats {
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_bytes: Option<i64>,
    pub network_rx_bytes: Option<i64>,
    pub network_tx_bytes: Option<i64>,
    pub volume_usage: Vec<VolumeUsage>,
}

#[async_trait::async_trait]
pub trait Runtime: Send + Sync {
    async fn pull_image(&self, image: &str) -> Result<()>;
    async fn start_workload(&self, spec: &WorkloadSpec) -> Result<String>;
    async fn inspect_status(&self, workload_handle: &str) -> Result<String>;
    fn logs(&self, workload_handle: &str) -> LogStream;
    async fn exec(&self, workload_handle: &str) -> Result<ExecSession>;
    async fn stop_workload(&self, workload_handle: &str) -> Result<()>;
    async fn stats(&self, workload_handle: &str) -> Result<ContainerStats>;

    async fn service_network_ip(
        &self,
        _workload_handle: &str,
        _network_name: &str,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    async fn list_managed_workloads(&self) -> Result<Vec<(String, String)>> {
        Ok(Vec::new())
    }
}
