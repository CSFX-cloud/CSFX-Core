use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateNetworkRequest {
    pub name: String,
    pub cidr: String,
    pub overlay_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePolicyRequest {
    pub direction: String,
    pub action: String,
    pub source_cidr: Option<String>,
    pub destination_cidr: Option<String>,
    pub port: Option<i32>,
    pub protocol: Option<String>,
    pub priority: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddMemberRequest {
    pub workload_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpamAllocation {
    pub network_id: uuid::Uuid,
    pub workload_id: uuid::Uuid,
    pub allocated_ip: String,
}
