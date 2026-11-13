use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::Utc;
use entity::{
    ingress_gateways, ingress_routes, site_deployments, site_domains, site_env_vars, sites,
    SiteDeployments, SiteDomains, SiteEnvVars, Sites,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use site_builder::{detect::Mode, validate};
use uuid::Uuid;

use super::ingress::validate_hostname;
use crate::{
    auth::{
        jwt::Claims,
        rbac::{CanManageResourceGroups, CanViewResourceGroups},
    },
    hostname_policy::{self, HostnameRejection},
    rbac_service::RbacService,
    site_service::{self as service, Detected, NewDeployment, Overrides, SecretSource, SiteError},
    vault_service, AppState,
};

const DEFAULT_BRANCH: &str = "main";
const DEPLOYMENT_PAGE_SIZE: u64 = 50;
const AUTO_DOMAIN_VARIABLE: &str = "CSFX_SITE_DOMAIN";
const INTERNAL_KIND: &str = "internal";
const MAX_DOMAINS_PER_SITE: u64 = 20;
const MAX_ENV_VARS_PER_SITE: u64 = 200;

type ApiResult<T> = Result<Json<T>, SiteError>;

fn invalid(message: &str) -> SiteError {
    SiteError::Invalid(message.to_string())
}

fn rejected(reason: Result<(), &'static str>) -> Result<(), SiteError> {
    reason.map_err(invalid)
}

pub(super) async fn load_site(
    state: &AppState,
    group_id: Uuid,
    site_id: Uuid,
) -> Result<sites::Model, SiteError> {
    vault_service::load_group(state, group_id).await?;
    Sites::find_by_id(site_id)
        .filter(sites::Column::ResourceGroupId.eq(group_id))
        .one(&state.db_conn)
        .await?
        .ok_or(SiteError::NotFound)
}

#[derive(Serialize)]
pub struct DeploymentSummary {
    id: Uuid,
    commit_sha: String,
    commit_message: String,
    branch: String,
    pull_request: Option<i32>,
    environment: String,
    status: String,
    duration_ms: Option<i64>,
    created_at: chrono::NaiveDateTime,
    finished_at: Option<chrono::NaiveDateTime>,
    current: bool,
    expired: bool,
}

fn summarize(model: site_deployments::Model, current: Option<Uuid>) -> DeploymentSummary {
    DeploymentSummary {
        current: current == Some(model.id),
        expired: model.status == "ready"
            && model.image_ref.is_none()
            && model.artifact_key.is_none(),
        id: model.id,
        commit_sha: model.commit_sha,
        commit_message: model.commit_message,
        branch: model.branch,
        pull_request: model.pull_request,
        environment: model.environment,
        status: model.status,
        duration_ms: model.duration_ms,
        created_at: model.created_at,
        finished_at: model.finished_at,
    }
}

#[derive(Serialize)]
pub struct EnvView {
    id: Uuid,
    name: String,
    value: Option<String>,
    vault_secret: Option<String>,
    scope: String,
}

#[derive(Serialize)]
pub struct SiteSummary {
    #[serde(flatten)]
    site: sites::Model,
    domains: Vec<String>,
    current: Option<DeploymentSummary>,
    latest: Option<DeploymentSummary>,
}

#[derive(Serialize)]
pub struct SiteDetail {
    #[serde(flatten)]
    site: sites::Model,
    #[serde(skip_serializing_if = "Option::is_none")]
    webhook_path: Option<String>,
    domains: Vec<site_domains::Model>,
    env: Vec<EnvView>,
    deployments: Vec<DeploymentSummary>,
    internal_port: Option<u16>,
}

async fn domains_of(
    state: &AppState,
    site_id: Uuid,
) -> Result<Vec<site_domains::Model>, SiteError> {
    Ok(SiteDomains::find()
        .filter(site_domains::Column::SiteId.eq(site_id))
        .order_by_asc(site_domains::Column::CreatedAt)
        .all(&state.db_conn)
        .await?)
}

async fn summarize_site(state: &AppState, site: sites::Model) -> Result<SiteSummary, SiteError> {
    let current = match site.current_deployment_id {
        Some(id) => SiteDeployments::find_by_id(id).one(&state.db_conn).await?,
        None => None,
    };
    let latest = SiteDeployments::find()
        .filter(site_deployments::Column::SiteId.eq(site.id))
        .order_by_desc(site_deployments::Column::CreatedAt)
        .one(&state.db_conn)
        .await?;
    let domains = domains_of(state, site.id).await?;
    Ok(SiteSummary {
        current: current.map(|model| summarize(model, site.current_deployment_id)),
        latest: latest.map(|model| summarize(model, site.current_deployment_id)),
        domains: domains
            .into_iter()
            .filter(|domain| domain.kind != INTERNAL_KIND)
            .map(|domain| domain.hostname)
            .collect(),
        site,
    })
}

pub async fn list_sites(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> ApiResult<Vec<SiteSummary>> {
    vault_service::load_group(&state, group_id).await?;
    let rows = Sites::find()
        .filter(sites::Column::ResourceGroupId.eq(group_id))
        .order_by_asc(sites::Column::Name)
        .all(&state.db_conn)
        .await?;
    let mut summaries = Vec::with_capacity(rows.len());
    for site in rows {
        summaries.push(summarize_site(&state, site).await?);
    }
    Ok(Json(summaries))
}

pub async fn get_site(
    CanViewResourceGroups(claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<SiteDetail> {
    let site = load_site(&state, group_id, site_id).await?;
    let env = SiteEnvVars::find()
        .filter(site_env_vars::Column::SiteId.eq(site.id))
        .order_by_asc(site_env_vars::Column::Name)
        .all(&state.db_conn)
        .await?
        .into_iter()
        .map(|row| EnvView {
            id: row.id,
            name: row.name,
            value: row.value,
            vault_secret: row.vault_secret,
            scope: row.scope,
        })
        .collect();
    let deployments = SiteDeployments::find()
        .filter(site_deployments::Column::SiteId.eq(site.id))
        .order_by_desc(site_deployments::Column::CreatedAt)
        .limit(DEPLOYMENT_PAGE_SIZE)
        .all(&state.db_conn)
        .await?
        .into_iter()
        .map(|model| summarize(model, site.current_deployment_id))
        .collect();
    let webhook_path = permitted(&state, &claims, "resource_groups", "manage")
        .await
        .then(|| format!("/hooks/sites/{}/{}", site.id, site.webhook_secret));
    Ok(Json(SiteDetail {
        webhook_path,
        domains: domains_of(&state, site.id).await?,
        env,
        deployments,
        internal_port: crate::ingress_sync::site_edge_port(),
        site,
    }))
}

#[derive(Deserialize)]
pub struct DetectRequest {
    repo_url: String,
    branch: Option<String>,
    root_dir: Option<String>,
    vault_id: Option<Uuid>,
    git_token_secret: Option<String>,
}

async fn permitted(state: &AppState, claims: &Claims, resource: &str, action: &str) -> bool {
    let Some(org_id) = state.default_org_id else {
        return false;
    };
    RbacService::new(state.db_conn.clone())
        .has_permission(claims.user_id, org_id, resource, action)
        .await
        .unwrap_or(false)
}

async fn require_reveal(state: &AppState, claims: &Claims) -> Result<(), SiteError> {
    if permitted(state, claims, "secrets", "reveal").await {
        Ok(())
    } else {
        Err(SiteError::Forbidden)
    }
}

async fn detect_remote(
    state: &AppState,
    group_id: Uuid,
    request: &DetectRequest,
) -> Result<Option<Detected>, SiteError> {
    let branch = request.branch.as_deref().unwrap_or(DEFAULT_BRANCH);
    let root_dir = request.root_dir.as_deref().unwrap_or_default();
    rejected(validate::repo_url(&request.repo_url))?;
    rejected(validate::branch(branch))?;
    rejected(validate::root_dir(root_dir))?;
    let token = match &request.git_token_secret {
        Some(name) => {
            let source = SecretSource {
                resource_group_id: group_id,
                vault_id: request.vault_id,
                actor: "site:detect".to_string(),
            };
            Some(service::fetch_secret(state, &source, name).await?)
        }
        None => None,
    };
    let builder = state.site_builder.as_ref().ok_or(SiteError::Unavailable)?;
    let body = json!({
        "repo_url": request.repo_url, "branch": branch, "root_dir": root_dir, "token": token
    });
    let response = builder.post("/internal/detect", &body).await?;
    serde_json::from_value(response["detection"].clone())
        .map_err(|_| invalid("detection returned an unexpected result"))
}

pub async fn detect_site(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<DetectRequest>,
) -> ApiResult<Value> {
    if request.git_token_secret.is_some() {
        require_reveal(&state, &claims).await?;
    }
    vault_service::load_group(&state, group_id).await?;
    let detected = detect_remote(&state, group_id, &request).await?;
    Ok(Json(json!({ "detection": detected.map(|found| json!({
        "framework": found.framework,
        "mode": found.mode,
        "install_command": found.install_command,
        "build_command": found.build_command,
        "output_dir": found.output_dir,
        "start_command": found.start_command,
        "port": found.port,
    })) })))
}

#[derive(Deserialize)]
pub struct CreateSiteRequest {
    name: String,
    #[serde(flatten)]
    source: DetectRequest,
    gateway_id: Option<Uuid>,
    preview_enabled: Option<bool>,
    #[serde(flatten)]
    overrides: Overrides,
}

async fn ensure_gateway(
    state: &AppState,
    group_id: Uuid,
    gateway_id: Uuid,
) -> Result<(), SiteError> {
    ingress_gateways::Entity::find_by_id(gateway_id)
        .filter(ingress_gateways::Column::ResourceGroupId.eq(group_id))
        .one(&state.db_conn)
        .await?
        .map(|_| ())
        .ok_or_else(|| invalid("the selected gateway does not exist in this resource group"))
}

async fn hostname_taken(state: &AppState, hostname: &str) -> Result<bool, SiteError> {
    let domains = SiteDomains::find()
        .filter(site_domains::Column::Hostname.eq(hostname))
        .count(&state.db_conn)
        .await?;
    let routes = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::Hostname.eq(hostname))
        .count(&state.db_conn)
        .await?;
    Ok(domains + routes > 0)
}

async fn insert_domain(
    state: &AppState,
    site_id: Uuid,
    hostname: String,
    kind: &str,
) -> Result<site_domains::Model, SiteError> {
    Ok(site_domains::ActiveModel {
        id: Set(Uuid::new_v4()),
        site_id: Set(site_id),
        hostname: Set(hostname),
        kind: Set(kind.to_string()),
        branch: Set(None),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await?)
}

fn webhook_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn validate_new(request: &CreateSiteRequest) -> Result<(), SiteError> {
    rejected(validate::site_name(&request.name))?;
    for command in [
        &request.overrides.install_command,
        &request.overrides.build_command,
        &request.overrides.start_command,
    ]
    .into_iter()
    .flatten()
    {
        rejected(validate::command(command))?;
    }
    Ok(())
}

async fn insert_site(
    state: &AppState,
    group_id: Uuid,
    request: &CreateSiteRequest,
    resolved: service::Resolved,
) -> Result<sites::Model, SiteError> {
    let mode = if resolved.mode == Mode::Static {
        "static"
    } else {
        "container"
    };
    Ok(sites::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group_id),
        name: Set(request.name.clone()),
        mode: Set(mode.to_string()),
        repo_url: Set(request.source.repo_url.clone()),
        production_branch: Set(request
            .source
            .branch
            .clone()
            .unwrap_or_else(|| DEFAULT_BRANCH.to_string())),
        root_dir: Set(request.source.root_dir.clone().unwrap_or_default()),
        framework: Set(resolved.framework),
        install_command: Set(resolved.install_command),
        build_command: Set(resolved.build_command),
        output_dir: Set(resolved.output_dir),
        start_command: Set(resolved.start_command),
        container_port: Set(resolved.container_port),
        vault_id: Set(request.source.vault_id),
        gateway_id: Set(request.gateway_id),
        git_token_secret: Set(request.source.git_token_secret.clone()),
        webhook_secret: Set(webhook_secret()),
        preview_enabled: Set(request.preview_enabled.unwrap_or(true)),
        created_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    }
    .insert(&state.db_conn)
    .await?)
}

fn internal_hostname(site_name: &str, group_id: Uuid) -> String {
    format!("{site_name}.svc.{group_id}.internal")
}

async fn claim_hostname(
    state: &AppState,
    site_id: Uuid,
    hostname: String,
    kind: &str,
) -> Result<(), SiteError> {
    if hostname_taken(state, &hostname).await? {
        return Err(SiteError::Conflict(
            "the generated domain is already in use",
        ));
    }
    insert_domain(state, site_id, hostname, kind).await?;
    Ok(())
}

async fn add_auto_domain(state: &AppState, site: &sites::Model) -> Result<(), SiteError> {
    let Some(domain) = std::env::var(AUTO_DOMAIN_VARIABLE)
        .ok()
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    claim_hostname(state, site.id, format!("{}.{domain}", site.name), "auto").await
}

async fn add_internal_domain(state: &AppState, site: &sites::Model) -> Result<(), SiteError> {
    if site.mode != "static" {
        return Ok(());
    }
    let hostname = internal_hostname(&site.name, site.resource_group_id);
    claim_hostname(state, site.id, hostname, INTERNAL_KIND).await
}

pub async fn backfill_auto_domains(state: &AppState) -> Result<(), SiteError> {
    for site in Sites::find().all(&state.db_conn).await? {
        if let Err(error) = backfill_site(state, &site).await {
            tracing::warn!(site = %site.id, error = %error, "internal domain backfill skipped");
        }
    }
    Ok(())
}

async fn backfill_site(state: &AppState, site: &sites::Model) -> Result<(), SiteError> {
    if site.mode != "static" {
        return Ok(());
    }
    let hostname = internal_hostname(&site.name, site.resource_group_id);
    let existing = SiteDomains::find()
        .filter(site_domains::Column::SiteId.eq(site.id))
        .filter(site_domains::Column::Kind.eq(INTERNAL_KIND))
        .one(&state.db_conn)
        .await?;
    match existing {
        Some(domain) if domain.hostname == hostname => return Ok(()),
        Some(domain) => {
            let mut active: site_domains::ActiveModel = domain.into();
            active.hostname = Set(hostname);
            active.update(&state.db_conn).await?;
        }
        None => claim_hostname(state, site.id, hostname, INTERNAL_KIND).await?,
    }
    reconcile_routes(state, site).await;
    tracing::info!(site = %site.id, "internal domain updated");
    Ok(())
}

async fn deploy_production(state: &AppState, site: &sites::Model) {
    let request = NewDeployment {
        branch: site.production_branch.clone(),
        sha: "HEAD".to_string(),
        message: "initial deployment".to_string(),
        environment: "production",
        pull_request: None,
    };
    if let Err(error) = service::queue(state, site, request).await {
        tracing::warn!(site = %site.id, error = %error, "initial deployment could not start");
    }
}

pub async fn create_site(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<CreateSiteRequest>,
) -> Result<(StatusCode, Json<sites::Model>), SiteError> {
    if request.source.git_token_secret.is_some() {
        require_reveal(&state, &claims).await?;
    }
    validate_new(&request)?;
    vault_service::load_group(&state, group_id).await?;
    if let Some(gateway_id) = request.gateway_id {
        ensure_gateway(&state, group_id, gateway_id).await?;
    }
    let taken = Sites::find()
        .filter(sites::Column::ResourceGroupId.eq(group_id))
        .filter(sites::Column::Name.eq(request.name.as_str()))
        .count(&state.db_conn)
        .await?;
    if taken > 0 {
        return Err(SiteError::Conflict("a site with this name already exists"));
    }
    let detected = detect_remote(&state, group_id, &request.source).await?;
    let resolved = service::resolve(&request.overrides, detected.as_ref())?;
    let site = insert_site(&state, group_id, &request, resolved).await?;
    add_internal_domain(&state, &site).await?;
    add_auto_domain(&state, &site).await?;
    deploy_production(&state, &site).await;
    Ok((StatusCode::CREATED, Json(site)))
}

#[derive(Deserialize)]
pub struct UpdateSiteRequest {
    production_branch: Option<String>,
    root_dir: Option<String>,
    framework: Option<String>,
    install_command: Option<String>,
    build_command: Option<String>,
    output_dir: Option<String>,
    start_command: Option<String>,
    container_port: Option<u16>,
    vault_id: Option<Uuid>,
    git_token_secret: Option<String>,
    gateway_id: Option<Uuid>,
    preview_enabled: Option<bool>,
}

fn clearable(value: Option<String>) -> ActiveValue<Option<String>> {
    match value {
        None => ActiveValue::NotSet,
        Some(text) if text.trim().is_empty() => Set(None),
        Some(text) => Set(Some(text)),
    }
}

fn validate_update(request: &UpdateSiteRequest) -> Result<(), SiteError> {
    if let Some(branch) = &request.production_branch {
        rejected(validate::branch(branch))?;
    }
    if let Some(root_dir) = &request.root_dir {
        rejected(validate::root_dir(root_dir))?;
    }
    for command in [
        &request.install_command,
        &request.build_command,
        &request.start_command,
    ]
    .into_iter()
    .flatten()
    {
        rejected(validate::command(command))?;
    }
    Ok(())
}

pub async fn update_site(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateSiteRequest>,
) -> ApiResult<sites::Model> {
    if request
        .git_token_secret
        .as_deref()
        .is_some_and(|name| !name.trim().is_empty())
    {
        require_reveal(&state, &claims).await?;
    }
    validate_update(&request)?;
    let site = load_site(&state, group_id, site_id).await?;
    if let Some(gateway_id) = request.gateway_id {
        ensure_gateway(&state, group_id, gateway_id).await?;
    }
    let mut active: sites::ActiveModel = site.into();
    if let Some(branch) = request.production_branch {
        active.production_branch = Set(branch);
    }
    if let Some(root_dir) = request.root_dir {
        active.root_dir = Set(root_dir);
    }
    active.framework = clearable(request.framework);
    active.install_command = clearable(request.install_command);
    active.build_command = clearable(request.build_command);
    active.output_dir = clearable(request.output_dir);
    active.start_command = clearable(request.start_command);
    active.git_token_secret = clearable(request.git_token_secret);
    if let Some(port) = request.container_port {
        active.container_port = Set(Some(i32::from(port)));
    }
    if let Some(vault_id) = request.vault_id {
        active.vault_id = Set(Some(vault_id));
    }
    if let Some(gateway_id) = request.gateway_id {
        active.gateway_id = Set(Some(gateway_id));
    }
    if let Some(enabled) = request.preview_enabled {
        active.preview_enabled = Set(enabled);
    }
    Ok(Json(active.update(&state.db_conn).await?))
}

pub async fn delete_site(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let workloads: Vec<Uuid> = SiteDeployments::find()
        .filter(site_deployments::Column::SiteId.eq(site.id))
        .filter(site_deployments::Column::WorkloadId.is_not_null())
        .all(&state.db_conn)
        .await?
        .into_iter()
        .filter_map(|deployment| deployment.workload_id)
        .collect();
    for workload in workloads {
        remove_workload(&state, workload).await?;
    }
    purge_images(&state, &site).await;
    Sites::delete_by_id(site.id).exec(&state.db_conn).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_workload(state: &AppState, workload: Uuid) -> Result<(), SiteError> {
    let path = format!("/workloads/{workload}");
    let (status, _) = state
        .service_client
        .forward_to_scheduler(reqwest::Method::DELETE, &path, None, None)
        .await
        .map_err(|error| {
            tracing::error!(workload = %workload, error = %error, "site workload removal failed");
            SiteError::Upstream("scheduler unavailable, site was not deleted")
        })?;
    if status.is_success() || status == StatusCode::NOT_FOUND {
        return Ok(());
    }
    tracing::error!(workload = %workload, status = status.as_u16(), "site workload removal rejected");
    Err(SiteError::Upstream(
        "scheduler failed to delete a site workload, site was not deleted",
    ))
}

async fn purge_images(state: &AppState, site: &sites::Model) {
    let Some(builder) = state.site_builder.as_ref() else {
        return;
    };
    let path = format!("/internal/sites/{}/purge", site.id);
    if let Err(error) = builder.post(&path, &Value::Null).await {
        tracing::warn!(site = %site.id, error = %error, "registry cleanup failed");
    }
}

#[derive(Deserialize, Default)]
pub struct RedeployRequest {
    branch: Option<String>,
}

pub async fn redeploy(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id)): Path<(Uuid, Uuid)>,
    body: Option<Json<RedeployRequest>>,
) -> Result<(StatusCode, Json<DeploymentSummary>), SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let branch = body
        .and_then(|Json(request)| request.branch)
        .unwrap_or_else(|| site.production_branch.clone());
    rejected(validate::branch(&branch))?;
    let production = branch == site.production_branch;
    if !production && site.mode != "static" {
        return Err(invalid("container sites only deploy the production branch"));
    }
    let request = NewDeployment {
        branch,
        sha: "HEAD".to_string(),
        message: "manual deployment".to_string(),
        environment: if production { "production" } else { "preview" },
        pull_request: None,
    };
    let deployment = service::queue(&state, &site, request).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(summarize(deployment, site.current_deployment_id)),
    ))
}

pub async fn get_deployment(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id, deployment_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<site_deployments::Model> {
    let site = load_site(&state, group_id, site_id).await?;
    SiteDeployments::find_by_id(deployment_id)
        .filter(site_deployments::Column::SiteId.eq(site.id))
        .one(&state.db_conn)
        .await?
        .map(Json)
        .ok_or(SiteError::NotFound)
}

pub async fn promote_deployment(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id, deployment_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<StatusCode, SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let deployment = SiteDeployments::find_by_id(deployment_id)
        .filter(site_deployments::Column::SiteId.eq(site.id))
        .one(&state.db_conn)
        .await?
        .ok_or(SiteError::NotFound)?;
    let env = if site.mode == "container" {
        service::environment(&state, &site, "production").await?
    } else {
        Default::default()
    };
    let builder = state.site_builder.as_ref().ok_or(SiteError::Unavailable)?;
    let path = format!("/internal/deployments/{}/promote", deployment.id);
    builder.post(&path, &json!({ "env": env })).await?;
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct EnvRequest {
    name: String,
    value: Option<String>,
    vault_secret: Option<String>,
    scope: Option<String>,
}

fn validate_env(request: &EnvRequest, site: &sites::Model) -> Result<String, SiteError> {
    rejected(validate::env_name(&request.name))?;
    let scope = request.scope.clone().unwrap_or_else(|| "all".to_string());
    if !["all", "production", "preview"].contains(&scope.as_str()) {
        return Err(invalid("scope must be all, production or preview"));
    }
    if request.value.is_some() == request.vault_secret.is_some() {
        return Err(invalid("provide either a value or a vault secret"));
    }
    if request.vault_secret.is_some() && site.vault_id.is_none() {
        return Err(invalid("the site has no key vault to read secrets from"));
    }
    Ok(scope)
}

pub async fn add_env(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<EnvRequest>,
) -> Result<(StatusCode, Json<EnvView>), SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let scope = validate_env(&request, &site)?;
    let existing = SiteEnvVars::find()
        .filter(site_env_vars::Column::SiteId.eq(site.id))
        .count(&state.db_conn)
        .await?;
    if existing >= MAX_ENV_VARS_PER_SITE {
        return Err(SiteError::Conflict("too many environment variables"));
    }
    let duplicate = SiteEnvVars::find()
        .filter(site_env_vars::Column::SiteId.eq(site.id))
        .filter(site_env_vars::Column::Name.eq(request.name.as_str()))
        .filter(site_env_vars::Column::Scope.eq(scope.as_str()))
        .count(&state.db_conn)
        .await?;
    if duplicate > 0 {
        return Err(SiteError::Conflict(
            "this variable already exists for the scope",
        ));
    }
    let row = site_env_vars::ActiveModel {
        id: Set(Uuid::new_v4()),
        site_id: Set(site.id),
        name: Set(request.name),
        value: Set(request.value),
        vault_secret: Set(request.vault_secret),
        scope: Set(scope),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await?;
    let view = EnvView {
        id: row.id,
        name: row.name,
        value: row.value,
        vault_secret: row.vault_secret,
        scope: row.scope,
    };
    Ok((StatusCode::CREATED, Json(view)))
}

pub async fn delete_env(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id, env_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<StatusCode, SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let removed = SiteEnvVars::delete_many()
        .filter(site_env_vars::Column::Id.eq(env_id))
        .filter(site_env_vars::Column::SiteId.eq(site.id))
        .exec(&state.db_conn)
        .await?;
    if removed.rows_affected == 0 {
        return Err(SiteError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DomainRequest {
    hostname: String,
}

async fn reconcile_routes(state: &AppState, site: &sites::Model) {
    let Some(builder) = state
        .site_builder
        .as_ref()
        .filter(|_| site.mode == "container")
    else {
        return;
    };
    let path = format!("/internal/sites/{}/reconcile", site.id);
    if let Err(error) = builder.post(&path, &Value::Null).await {
        tracing::warn!(site = %site.id, error = %error, "route reconciliation failed");
    }
}

pub async fn add_domain(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<DomainRequest>,
) -> Result<(StatusCode, Json<site_domains::Model>), SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let hostname = validate_hostname(&request.hostname).map_err(invalid)?;
    let count = SiteDomains::find()
        .filter(site_domains::Column::SiteId.eq(site.id))
        .count(&state.db_conn)
        .await?;
    if count >= MAX_DOMAINS_PER_SITE {
        return Err(SiteError::Conflict("too many domains"));
    }
    hostname_policy::ensure_claimable(&state.db_conn, &hostname)
        .await
        .map_err(|rejection| match rejection {
            HostnameRejection::Database(e) => SiteError::Database(e),
            other => SiteError::Conflict(other.message()),
        })?;
    let domain = insert_domain(&state, site.id, hostname, "production").await?;
    reconcile_routes(&state, &site).await;
    Ok((StatusCode::CREATED, Json(domain)))
}

pub async fn delete_domain(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, site_id, domain_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<StatusCode, SiteError> {
    let site = load_site(&state, group_id, site_id).await?;
    let removed = SiteDomains::delete_many()
        .filter(site_domains::Column::Id.eq(domain_id))
        .filter(site_domains::Column::SiteId.eq(site.id))
        .filter(site_domains::Column::Kind.ne(INTERNAL_KIND))
        .exec(&state.db_conn)
        .await?;
    if removed.rows_affected == 0 {
        return Err(SiteError::NotFound);
    }
    reconcile_routes(&state, &site).await;
    Ok(StatusCode::NO_CONTENT)
}

fn hook_event_name(headers: &HeaderMap) -> &str {
    ["x-github-event", "x-forgejo-event", "x-gitea-event"]
        .iter()
        .find_map(|name| headers.get(*name))
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

fn hook_deployment(site: &sites::Model, event: service::HookEvent) -> Option<NewDeployment> {
    let production = event.pull_request.is_none() && event.branch == site.production_branch;
    let preview = event.pull_request.is_some() && site.preview_enabled && site.mode == "static";
    if !production && !preview {
        return None;
    }
    Some(NewDeployment {
        branch: event.branch,
        sha: event.sha,
        message: event.message,
        environment: if production { "production" } else { "preview" },
        pull_request: event.pull_request,
    })
}

pub async fn webhook(
    State(state): State<AppState>,
    Path((site_id, secret)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<(StatusCode, Json<Value>), SiteError> {
    let site = Sites::find_by_id(site_id)
        .one(&state.db_conn)
        .await?
        .filter(|site| service::secret_matches(&site.webhook_secret, &secret))
        .ok_or(SiteError::NotFound)?;
    let event = service::parse_hook(hook_event_name(&headers), &payload);
    let Some(request) = event.and_then(|event| hook_deployment(&site, event)) else {
        return Ok((StatusCode::OK, Json(json!({ "ignored": true }))));
    };
    let deployment = service::queue(&state, &site, request).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "deployment_id": deployment.id })),
    ))
}

pub fn sites_routes() -> Router<AppState> {
    let base = "/resource-groups/{group_id}/sites";
    let site = "/resource-groups/{group_id}/sites/{site_id}";
    Router::new()
        .route(base, get(list_sites).post(create_site))
        .route(&format!("{base}/detect"), post(detect_site))
        .route(site, get(get_site).put(update_site).delete(delete_site))
        .route(&format!("{site}/deployments"), post(redeploy))
        .route(
            &format!("{site}/deployments/{{deployment_id}}"),
            get(get_deployment),
        )
        .route(
            &format!("{site}/deployments/{{deployment_id}}/promote"),
            post(promote_deployment),
        )
        .route(&format!("{site}/env"), post(add_env))
        .route(&format!("{site}/env/{{env_id}}"), delete(delete_env))
        .route(&format!("{site}/domains"), post(add_domain))
        .route(
            &format!("{site}/domains/{{domain_id}}"),
            delete(delete_domain),
        )
}

pub fn sites_hook_routes() -> Router<AppState> {
    Router::new().route("/hooks/sites/{site_id}/{secret}", post(webhook))
}

#[cfg(test)]
mod tests {
    use super::internal_hostname;
    use uuid::Uuid;

    #[test]
    fn internal_hostname_uses_the_service_zone_of_the_group() {
        let group = Uuid::from_u128(0x99aec6da_b859_419d_9dea_648b8a87895e);
        assert_eq!(
            internal_hostname("docs", group),
            "docs.svc.99aec6da-b859-419d-9dea-648b8a87895e.internal"
        );
    }
}
