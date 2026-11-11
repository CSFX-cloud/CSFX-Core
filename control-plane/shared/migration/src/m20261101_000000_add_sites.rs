use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_SITES: &str = "
CREATE TABLE IF NOT EXISTS sites (
    id UUID PRIMARY KEY,
    resource_group_id UUID NOT NULL REFERENCES resource_groups (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('static', 'container')),
    repo_url TEXT NOT NULL,
    production_branch TEXT NOT NULL DEFAULT 'main',
    root_dir TEXT NOT NULL DEFAULT '',
    framework TEXT,
    install_command TEXT,
    build_command TEXT,
    output_dir TEXT,
    start_command TEXT,
    container_port INTEGER,
    vault_id UUID REFERENCES vaults (id) ON DELETE SET NULL,
    git_token_secret TEXT,
    webhook_secret TEXT NOT NULL,
    preview_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    current_deployment_id UUID,
    created_at TIMESTAMP NOT NULL,
    UNIQUE (resource_group_id, name)
)";

const CREATE_SITE_DEPLOYMENTS: &str = "
CREATE TABLE IF NOT EXISTS site_deployments (
    id UUID PRIMARY KEY,
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    commit_sha TEXT NOT NULL,
    commit_message TEXT NOT NULL DEFAULT '',
    branch TEXT NOT NULL,
    pull_request INTEGER,
    environment TEXT NOT NULL CHECK (environment IN ('production', 'preview')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'building', 'ready', 'failed', 'rolled_back')),
    artifact_key TEXT,
    image_ref TEXT,
    workload_id UUID,
    build_log TEXT NOT NULL DEFAULT '',
    duration_ms BIGINT,
    created_at TIMESTAMP NOT NULL,
    finished_at TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_site_deployments_site_time ON site_deployments (site_id, created_at DESC)";

const CREATE_SITE_ENV_VARS: &str = "
CREATE TABLE IF NOT EXISTS site_env_vars (
    id UUID PRIMARY KEY,
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    value TEXT,
    vault_secret TEXT,
    scope TEXT NOT NULL DEFAULT 'all' CHECK (scope IN ('all', 'production', 'preview')),
    created_at TIMESTAMP NOT NULL,
    UNIQUE (site_id, name, scope),
    CHECK ((value IS NULL) <> (vault_secret IS NULL))
)";

const CREATE_SITE_DOMAINS: &str = "
CREATE TABLE IF NOT EXISTS site_domains (
    id UUID PRIMARY KEY,
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    hostname TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('production', 'branch', 'auto')),
    branch TEXT,
    created_at TIMESTAMP NOT NULL
)";

const DROP_SITES: &str = "
DROP TABLE IF EXISTS site_domains;
DROP TABLE IF EXISTS site_env_vars;
DROP TABLE IF EXISTS site_deployments;
DROP TABLE IF EXISTS sites";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        for statement in [
            CREATE_SITES,
            CREATE_SITE_DEPLOYMENTS,
            CREATE_SITE_ENV_VARS,
            CREATE_SITE_DOMAINS,
        ] {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DROP_SITES)
            .await?;
        Ok(())
    }
}
