use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CephVolume {
    pub name: String,
    pub pool: String,
    pub size_mb: u64,
    pub features: Vec<String>,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CephPool {
    pub name: String,
    pub pg_num: u32,
    pub pgp_num: u32,
    pub size: u32, // Replikation
    pub min_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CephClusterHealth {
    pub status: HealthStatus,
    pub mons: Vec<MonitorInfo>,
    pub osds: Vec<OsdInfo>,
    pub pgs: PgSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub name: String,
    pub addr: String,
    pub rank: u32,
    pub in_quorum: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdInfo {
    pub id: u32,
    pub up: bool,
    pub in_cluster: bool,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgSummary {
    pub total: u32,
    pub active_clean: u32,
    pub degraded: u32,
    pub misplaced: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbdImage {
    pub name: String,
    pub size: u64,
    pub pool: String,
    pub format: u32,
    pub features: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CephCommand {
    pub cmd: String,
    pub args: Vec<String>,
}

impl CephCommand {
    pub fn new(cmd: impl Into<String>) -> Self {
        Self {
            cmd: cmd.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args_vec(mut self, args: Vec<String>) -> Self {
        self.args.extend(args);
        self
    }

    pub fn to_vec(&self) -> Vec<String> {
        let mut result = vec![self.cmd.clone()];
        result.extend(self.args.clone());
        result
    }
}
