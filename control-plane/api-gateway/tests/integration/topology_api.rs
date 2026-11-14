use axum::http::{Method, StatusCode};
use serde_json::Value;

use crate::common::{harness, Harness};
use crate::support_a::{
    insert_agent, insert_group, insert_metrics, insert_volume, insert_workload, WorkloadSpec,
};

fn has_node(topology: &Value, id: &str, kind: &str) -> bool {
    topology["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["id"] == id && node["kind"] == kind)
}

fn has_edge(topology: &Value, kind: &str, source: &str, target: &str) -> bool {
    topology["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|edge| edge["kind"] == kind && edge["source"] == source && edge["target"] == target)
}

async fn push_metrics(harness: &Harness, agent_id: uuid::Uuid) {
    insert_metrics(&harness.db, agent_id, Some(12.5), Some(40.0), Some(55.0)).await;
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn topology_requires_every_view_permission() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    let (status, _) = harness
        .send(Method::GET, "/api/topology", Some(&limited), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = harness.send(Method::GET, "/api/topology", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn topology_links_cluster_agents_workloads_and_volumes() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    push_metrics(&harness, agent.id).await;
    let hosted = insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(agent.id), "running"),
    )
    .await;
    let waiting = insert_workload(&harness.db, WorkloadSpec::new(group.id, None, "pending")).await;
    let deleted = insert_workload(&harness.db, WorkloadSpec::new(group.id, None, "deleted")).await;
    let mounted = insert_volume(&harness.db, None, Some(hosted.id)).await;
    let attached = insert_volume(&harness.db, Some(agent.id), None).await;
    let loose = insert_volume(&harness.db, None, None).await;

    let (status, topology) = harness.get("/api/topology").await;
    assert_eq!(status, StatusCode::OK);
    assert!(has_node(&topology, "cluster", "cluster"));
    assert!(has_node(
        &topology,
        &format!("rg:{}", group.id),
        "resource_group"
    ));
    assert!(has_node(&topology, &format!("agent:{}", agent.id), "agent"));
    assert!(has_node(
        &topology,
        &format!("workload:{}", hosted.id),
        "workload"
    ));
    assert!(!has_node(
        &topology,
        &format!("workload:{}", deleted.id),
        "workload"
    ));
    let agent_node = topology["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == format!("agent:{}", agent.id))
        .unwrap();
    assert_eq!(agent_node["data"]["cpu_percent"], 12.5);

    let agent_id = format!("agent:{}", agent.id);
    assert!(has_edge(&topology, "member", "cluster", &agent_id));
    assert!(has_edge(
        &topology,
        "runs",
        &agent_id,
        &format!("workload:{}", hosted.id)
    ));
    assert!(has_edge(
        &topology,
        "unscheduled",
        "cluster",
        &format!("workload:{}", waiting.id)
    ));
    assert!(has_edge(
        &topology,
        "mounts",
        &format!("workload:{}", hosted.id),
        &format!("volume:{}", mounted.id)
    ));
    assert!(has_edge(
        &topology,
        "attached",
        &agent_id,
        &format!("volume:{}", attached.id)
    ));
    assert!(has_edge(
        &topology,
        "unattached",
        "cluster",
        &format!("volume:{}", loose.id)
    ));
}
