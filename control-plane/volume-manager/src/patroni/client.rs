use super::types::*;
use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

pub struct PatroniClient {
    client: Client,
    scope: String,
    nodes: Vec<String>, // API URLs like "http://patroni1:8008"
}

impl PatroniClient {
    pub fn new(scope: String, nodes: Vec<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        Self {
            client,
            scope,
            nodes,
        }
    }

    /// Holt den Cluster-Status von allen Patroni Nodes
    pub async fn get_cluster_status(&self) -> Result<PatroniCluster> {
        crate::log_debug!("patroni", "Fetching cluster status");

        let mut members = Vec::new();
        let mut leader = None;

        for node_url in &self.nodes {
            match self.get_node_health(node_url).await {
                Ok(health) => {
                    let role = PostgresNodeRole::from(health.role.as_str());

                    if role == PostgresNodeRole::Primary {
                        leader = Some(self.extract_node_name(node_url));
                    }

                    members.push(PatroniNode {
                        name: self.extract_node_name(node_url),
                        role,
                        state: PatroniState::from(health.state.as_str()),
                        api_url: node_url.clone(),
                        postgres_url: self.build_postgres_url(node_url),
                        timeline: health.timeline,
                        lag: None, // Wird später von Cluster-API gefüllt
                    });
                }
                Err(e) => {
                    crate::log_warn!(
                        "patroni",
                        &format!("Failed to get health from {}: {}", node_url, e)
                    );

                    members.push(PatroniNode {
                        name: self.extract_node_name(node_url),
                        role: PostgresNodeRole::Unknown,
                        state: PatroniState::Failed,
                        api_url: node_url.clone(),
                        postgres_url: self.build_postgres_url(node_url),
                        timeline: None,
                        lag: None,
                    });
                }
            }
        }

        Ok(PatroniCluster {
            scope: self.scope.clone(),
            members,
            leader,
            failover_in_progress: false,
        })
    }

    /// Holt Health-Info von einem einzelnen Node
    async fn get_node_health(&self, node_url: &str) -> Result<PatroniHealth> {
        let url = format!("{}/health", node_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send health request")?;

        let health: PatroniHealth = response
            .json()
            .await
            .context("Failed to parse health response")?;

        crate::log_debug!(
            "patroni",
            &format!(
                "Fetched health from {}: role={}, state={}",
                node_url, health.role, health.state
            )
        );

        Ok(health)
    }

    /// Prüft ob ein Node der Primary ist
    pub async fn is_primary(&self, node_url: &str) -> Result<bool> {
        let url = format!("{}/primary", node_url);

        let response = self.client.get(&url).send().await?;

        let is_primary = response.status().is_success();
        crate::log_debug!(
            "patroni",
            &format!("Node {} is primary: {}", node_url, is_primary)
        );

        Ok(is_primary)
    }

    /// Prüft ob ein Node ein Replica ist
    pub async fn is_replica(&self, node_url: &str) -> Result<bool> {
        let url = format!("{}/replica", node_url);

        let response = self.client.get(&url).send().await?;

        let is_replica = response.status().is_success();
        crate::log_debug!(
            "patroni",
            &format!("Node {} is replica: {}", node_url, is_replica)
        );

        Ok(is_replica)
    }

    /// Findet die aktuelle Primary Node
    pub async fn find_primary(&self) -> Result<Option<PatroniNode>> {
        let cluster = self.get_cluster_status().await?;

        let primary = cluster
            .members
            .into_iter()
            .find(|m| m.role == PostgresNodeRole::Primary);

        if let Some(ref p) = primary {
            crate::log_info!("patroni", &format!("Found primary node: {}", p.name));
        } else {
            crate::log_warn!("patroni", "No primary node found in cluster");
        }

        Ok(primary)
    }

    /// Holt alle Replica Nodes
    pub async fn find_replicas(&self) -> Result<Vec<PatroniNode>> {
        let cluster = self.get_cluster_status().await?;

        let replicas: Vec<PatroniNode> = cluster
            .members
            .into_iter()
            .filter(|m| m.role == PostgresNodeRole::Replica)
            .collect();

        crate::log_info!(
            "patroni",
            &format!("Found {} replica nodes", replicas.len())
        );

        Ok(replicas)
    }

    /// Triggert ein manuelles Failover (NUR FÜR TESTING!)
    pub async fn trigger_failover(&self, candidate: Option<&str>) -> Result<()> {
        crate::log_warn!(
            "patroni",
            &format!("Triggering manual failover to {:?}", candidate)
        );

        // Finde Primary
        let primary = self.find_primary().await?.context("No primary found")?;

        let url = format!("{}/failover", primary.api_url);

        let mut body = serde_json::json!({
            "leader": self.extract_node_name(&primary.api_url),
        });

        if let Some(candidate) = candidate {
            body["candidate"] = serde_json::json!(candidate);
        }

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to trigger failover")?;

        crate::log_info!("patroni", "Failover triggered successfully");
        Ok(())
    }

    /// Extrahiert Node-Namen aus URL
    fn extract_node_name(&self, url: &str) -> String {
        url.split("://")
            .nth(1)
            .and_then(|s| s.split(':').next())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Baut PostgreSQL Connection URL
    fn build_postgres_url(&self, api_url: &str) -> String {
        let host = self.extract_node_name(api_url);
        format!("postgresql://{}:5432", host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_node_name() {
        let client =
            PatroniClient::new("test".to_string(), vec!["http://patroni1:8008".to_string()]);

        assert_eq!(client.extract_node_name("http://patroni1:8008"), "patroni1");
        assert_eq!(
            client.extract_node_name("http://192.168.1.100:8008"),
            "192.168.1.100"
        );
    }

    #[test]
    fn test_build_postgres_url() {
        let client =
            PatroniClient::new("test".to_string(), vec!["http://patroni1:8008".to_string()]);

        assert_eq!(
            client.build_postgres_url("http://patroni1:8008"),
            "postgresql://patroni1:5432"
        );
    }
}
