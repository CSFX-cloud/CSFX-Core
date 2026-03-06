use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    db::networks as db,
    models::{AddMemberRequest, CreateNetworkRequest, CreatePolicyRequest},
    server::AppState,
};

pub async fn create_network(
    State(state): State<AppState>,
    Json(req): Json<CreateNetworkRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match db::create_network(&state.db, req).await {
        Ok(network) => Ok((StatusCode::CREATED, Json(json!(network)))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create network");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn list_networks(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match db::list_networks(&state.db).await {
        Ok(networks) => Ok(Json(json!(networks))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list networks");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn get_network(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match db::get_network(&state.db, id).await {
        Ok(Some(network)) => Ok(Json(json!(network))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({"error": "Network not found"})))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get network");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn delete_network(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match db::delete_network(&state.db, id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete network");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn create_policy(
    State(state): State<AppState>,
    Path(network_id): Path<Uuid>,
    Json(req): Json<CreatePolicyRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match db::create_policy(&state.db, network_id, req).await {
        Ok(policy) => Ok((StatusCode::CREATED, Json(json!(policy)))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create policy");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn list_policies(
    State(state): State<AppState>,
    Path(network_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match db::list_policies(&state.db, network_id).await {
        Ok(policies) => Ok(Json(json!(policies))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list policies");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn add_member(
    State(mut state): State<AppState>,
    Path(network_id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let network = match db::get_network(&state.db, network_id).await {
        Ok(Some(n)) => n,
        Ok(None) => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Network not found"})))),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))),
    };

    let allocated_ip = match state.ipam.allocate(network_id, &network.cidr, req.workload_id).await {
        Ok(ip) => ip,
        Err(e) => {
            tracing::error!(error = %e, "IPAM allocation failed");
            return Err((StatusCode::CONFLICT, Json(json!({"error": e.to_string()}))));
        }
    };

    match db::add_member(&state.db, network_id, req.workload_id, allocated_ip).await {
        Ok(member) => Ok((StatusCode::CREATED, Json(json!(member)))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to add network member");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn list_members(
    State(state): State<AppState>,
    Path(network_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match db::list_members(&state.db, network_id).await {
        Ok(members) => Ok(Json(json!(members))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list network members");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}

pub async fn remove_member(
    State(mut state): State<AppState>,
    Path((network_id, workload_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let members = db::list_members(&state.db, network_id).await.unwrap_or_default();
    let ip = members
        .iter()
        .find(|m| m.workload_id == workload_id)
        .map(|m| m.allocated_ip.clone());

    if let Some(ip) = ip {
        let _ = state.ipam.release(network_id, &ip).await;
    }

    match db::remove_member(&state.db, network_id, workload_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            tracing::error!(error = %e, "Failed to remove network member");
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))
        }
    }
}
