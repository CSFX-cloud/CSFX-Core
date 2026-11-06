use scheduler::db::workloads;
use scheduler::models::workload::{CreateWorkloadRequest, WorkloadStatus};
use serde_json::json;
use uuid::Uuid;

use crate::common::{
    insert_agent, insert_resource_group, placing, small_request, unique_name, Placing,
};

async fn placement_key_exists(fixture: &Placing, workload_id: Uuid) -> bool {
    let key = format!("/csfx/placements/{workload_id}");
    let response = fixture.etcd.lock().await.get(key, None).await.unwrap();
    !response.kvs().is_empty()
}

fn request_with_ports(name: &str) -> CreateWorkloadRequest {
    serde_json::from_value(json!({
        "name": name,
        "image": "nginx:latest",
        "cpu_millicores": 10,
        "memory_bytes": 1048576,
        "disk_bytes": 1048576,
        "ports": [{"container_port": 80, "protocol": "tcp"}]
    }))
    .unwrap()
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn schedulable_workloads_are_assigned_and_recorded() {
    let fixture = placing().await;

    let response = fixture
        .service
        .schedule(small_request(&unique_name("placed")))
        .await
        .unwrap();

    assert!(matches!(response.status, WorkloadStatus::Scheduled));
    let agent_id = response.assigned_agent_id.expect("agent must be assigned");
    assert!(response.message.contains(&agent_id.to_string()));
    let stored = workloads::get_by_id(&fixture.db, response.workload_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.assigned_agent_id, Some(agent_id));
    assert!(placement_key_exists(&fixture, response.workload_id).await);
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn node_ports_are_allocated_on_placement() {
    let fixture = placing().await;

    let response = fixture
        .service
        .schedule(request_with_ports(&unique_name("ports")))
        .await
        .unwrap();

    let stored = workloads::get_by_id(&fixture.db, response.workload_id)
        .await
        .unwrap()
        .unwrap();
    let ports = stored.ports.expect("ports must be stored");
    assert!(ports[0]["node_port"].as_u64().is_some());
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn replicas_are_all_placed() {
    let fixture = placing().await;
    let mut request = small_request(&unique_name("replicated"));
    request.replicas = Some(3);

    let responses = fixture.service.schedule_replicas(request).await.unwrap();

    assert_eq!(responses.len(), 3);
    assert!(responses
        .iter()
        .all(|response| response.assigned_agent_id.is_some()));
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn invalid_requests_never_reach_the_database() {
    let fixture = placing().await;
    let mut request = small_request(&unique_name("invalid"));
    request.cpu_millicores = -1;

    assert!(fixture.service.schedule(request.clone()).await.is_err());
    assert!(fixture.service.schedule_replicas(request).await.is_err());
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn deleting_a_workload_removes_its_placement() {
    let fixture = placing().await;
    let response = fixture
        .service
        .schedule(small_request(&unique_name("doomed")))
        .await
        .unwrap();

    fixture
        .service
        .delete_workload(response.workload_id)
        .await
        .unwrap();

    assert!(!placement_key_exists(&fixture, response.workload_id).await);
    assert!(workloads::get_by_id(&fixture.db, response.workload_id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn pending_workloads_are_placed_by_the_retry_loop() {
    let fixture = placing().await;
    let pending = workloads::create(&fixture.db, &small_request(&unique_name("retry")), None)
        .await
        .unwrap();
    assert!(pending.assigned_agent_id.is_none());

    for _ in 0..5 {
        if fixture.service.retry_pending().await.is_ok() {
            break;
        }
    }

    let stored = workloads::get_by_id(&fixture.db, pending.id)
        .await
        .unwrap()
        .unwrap();
    assert!(stored.assigned_agent_id.is_some());
    assert!(placement_key_exists(&fixture, pending.id).await);
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn workloads_move_off_a_dead_agent() {
    let fixture = placing().await;
    let first = fixture
        .service
        .schedule(small_request(&unique_name("survivor")))
        .await
        .unwrap();
    let dead_agent = first.assigned_agent_id.unwrap();
    insert_agent(&fixture.db).await;

    let moved = fixture
        .service
        .reschedule_from_agent(dead_agent, &[first.workload_id])
        .await
        .unwrap();

    assert_eq!(moved.len(), 1);
    let target = moved[0]
        .assigned_agent_id
        .expect("a new agent must be chosen");
    assert_ne!(target, dead_agent);
    let stored = workloads::get_by_id(&fixture.db, first.workload_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.assigned_agent_id, Some(target));
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn rescheduling_an_unknown_workload_fails() {
    let fixture = placing().await;

    let error = fixture
        .service
        .reschedule_from_agent(Uuid::new_v4(), &[Uuid::new_v4()])
        .await
        .unwrap_err();

    assert!(error.contains("not found"));
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn guest_ips_are_assigned_within_the_resource_group() {
    let fixture = placing().await;
    let group = insert_resource_group(&fixture.db).await;
    let mut request = small_request(&unique_name("addressed"));
    request.resource_group_id = Some(group);

    let first = fixture.service.schedule(request.clone()).await.unwrap();
    request.name = unique_name("addressed");
    let second = fixture.service.schedule(request).await.unwrap();

    let first = workloads::get_by_id(&fixture.db, first.workload_id)
        .await
        .unwrap()
        .unwrap();
    let second = workloads::get_by_id(&fixture.db, second.workload_id)
        .await
        .unwrap()
        .unwrap();
    assert!(first.guest_ip.is_some());
    assert_ne!(first.guest_ip, second.guest_ip);
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn missing_guest_ips_are_backfilled() {
    let fixture = placing().await;
    let group = insert_resource_group(&fixture.db).await;
    let mut request = small_request(&unique_name("legacy"));
    request.resource_group_id = Some(group);
    let legacy = workloads::create(&fixture.db, &request, None)
        .await
        .unwrap();

    let assigned = fixture.service.backfill_guest_ips().await.unwrap();

    assert!(assigned >= 1);
    let stored = workloads::get_by_id(&fixture.db, legacy.id)
        .await
        .unwrap()
        .unwrap();
    assert!(stored.guest_ip.is_some());
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn stacks_are_placed_updated_and_removed() {
    let fixture = placing().await;
    let group = insert_resource_group(&fixture.db).await;
    let compose = "services:\n  web:\n    image: nginx:latest\n  cache:\n    image: redis:7\n";

    let created = fixture
        .service
        .schedule_stack(scheduler::models::compose::CreateStackRequest {
            name: unique_name("stack"),
            resource_group_id: group,
            compose_yaml: compose.to_string(),
        })
        .await
        .unwrap();

    assert_eq!(created.workloads.len(), 2);
    assert!(created
        .workloads
        .iter()
        .all(|workload| workload.assigned_agent_id.is_some()));

    fixture.service.stop_stack(created.stack_id).await.unwrap();
    fixture
        .service
        .restart_stack(created.stack_id)
        .await
        .unwrap();
    fixture
        .service
        .redeploy_stack(
            created.stack_id,
            "services:\n  web:\n    image: nginx:1.27\n",
        )
        .await
        .unwrap();
    let members = workloads::get_by_stack_id(&fixture.db, created.stack_id)
        .await
        .unwrap();
    assert!(members.iter().any(|member| member.image == "nginx:1.27"));

    fixture
        .service
        .delete_stack(created.stack_id)
        .await
        .unwrap();
    assert!(workloads::get_by_stack_id(&fixture.db, created.stack_id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
#[ignore = "requires postgres and etcd"]
async fn redeploying_an_unknown_stack_fails() {
    let fixture = placing().await;

    let error = fixture
        .service
        .redeploy_stack(Uuid::new_v4(), "services:\n  web:\n    image: nginx\n")
        .await
        .unwrap_err();

    assert!(error.contains("not found"));
}
