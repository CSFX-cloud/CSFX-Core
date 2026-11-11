use crate::error::ServiceError;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct SchedulerClient {
    client: Client,
    base: String,
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Scheduler(error.to_string())
}

impl SchedulerClient {
    pub fn new(base: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder().timeout(REQUEST_TIMEOUT).build()?,
            base: base.trim_end_matches('/').to_string(),
        })
    }

    pub async fn create(&self, request: &Value) -> Result<Uuid, ServiceError> {
        let response = self
            .client
            .post(format!("{}/workloads", self.base))
            .json(request)
            .send()
            .await
            .map_err(failure)?;
        let status = response.status();
        let body: Value = response.json().await.map_err(failure)?;
        if !status.is_success() {
            return Err(failure(
                body["error"].as_str().unwrap_or("request rejected"),
            ));
        }
        body["workload_id"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| failure("response carried no workload id"))
    }

    pub async fn delete(&self, workload_id: Uuid) -> Result<(), ServiceError> {
        let response = self
            .client
            .delete(format!("{}/workloads/{workload_id}", self.base))
            .send()
            .await
            .map_err(failure)?;
        if response.status().is_success() || response.status().as_u16() == 404 {
            return Ok(());
        }
        Err(failure(format!("delete returned {}", response.status())))
    }

    pub async fn status(&self, workload_id: Uuid) -> Result<Option<String>, ServiceError> {
        let workloads: Vec<Value> = self
            .client
            .get(format!("{}/workloads", self.base))
            .send()
            .await
            .map_err(failure)?
            .json()
            .await
            .map_err(failure)?;
        let wanted = workload_id.to_string();
        Ok(workloads
            .iter()
            .find(|workload| workload["id"].as_str() == Some(wanted.as_str()))
            .and_then(|workload| workload["status"].as_str())
            .map(str::to_string))
    }
}
