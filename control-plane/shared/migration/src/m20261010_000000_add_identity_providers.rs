use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE "user" ADD COLUMN auth_source VARCHAR NOT NULL DEFAULT 'local';

CREATE TABLE identity_providers (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
    slug VARCHAR NOT NULL UNIQUE,
    display_name VARCHAR NOT NULL,
    issuer_url VARCHAR NOT NULL,
    client_id VARCHAR NOT NULL,
    client_secret_encrypted TEXT NOT NULL,
    scopes VARCHAR NOT NULL DEFAULT 'openid profile email',
    username_claim VARCHAR NOT NULL DEFAULT 'preferred_username',
    email_claim VARCHAR NOT NULL DEFAULT 'email',
    groups_claim VARCHAR NOT NULL DEFAULT 'groups',
    default_role_id UUID REFERENCES role(id) ON DELETE SET NULL,
    auto_provision BOOLEAN NOT NULL DEFAULT TRUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE idp_group_mappings (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES identity_providers(id) ON DELETE CASCADE,
    external_group VARCHAR NOT NULL,
    role_id UUID NOT NULL REFERENCES role(id) ON DELETE CASCADE,
    priority INTEGER NOT NULL DEFAULT 0,
    UNIQUE (provider_id, external_group)
);

CREATE TABLE user_identities (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES identity_providers(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    subject VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    UNIQUE (provider_id, subject)
);

CREATE TABLE oidc_login_states (
    state VARCHAR PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES identity_providers(id) ON DELETE CASCADE,
    nonce VARCHAR NOT NULL,
    pkce_verifier VARCHAR NOT NULL,
    expires_at TIMESTAMP NOT NULL
);

CREATE TABLE oidc_exchange_codes (
    code_hash VARCHAR PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    expires_at TIMESTAMP NOT NULL
);
"#;

const DOWN: &str = r#"
DROP TABLE oidc_exchange_codes;
DROP TABLE oidc_login_states;
DROP TABLE user_identities;
DROP TABLE idp_group_mappings;
DROP TABLE identity_providers;
ALTER TABLE "user" DROP COLUMN auth_source;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(DOWN).await?;
        Ok(())
    }
}
