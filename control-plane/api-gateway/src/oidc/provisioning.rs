use chrono::Utc;
use entity::{
    identity_providers, idp_group_mappings, user, user_identities, IdpGroupMappings, User,
    UserIdentities,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};
use tracing::info;
use uuid::Uuid;

use super::{error::OidcError, identity::ExternalIdentity};
use crate::rbac_service::RbacService;

const AUTH_SOURCE: &str = "oidc";

async fn resolve_role(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
    groups: &[String],
) -> Result<Uuid, OidcError> {
    let mapped = IdpGroupMappings::find()
        .filter(idp_group_mappings::Column::ProviderId.eq(provider.id))
        .filter(idp_group_mappings::Column::ExternalGroup.is_in(groups.iter().cloned()))
        .order_by_desc(idp_group_mappings::Column::Priority)
        .one(db)
        .await?;
    mapped
        .map(|mapping| mapping.role_id)
        .or(provider.default_role_id)
        .ok_or(OidcError::AccessDenied("no_role"))
}

async fn existing_user(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
    subject: &str,
) -> Result<Option<user::Model>, OidcError> {
    let identity = UserIdentities::find()
        .filter(user_identities::Column::ProviderId.eq(provider.id))
        .filter(user_identities::Column::Subject.eq(subject))
        .one(db)
        .await?;
    match identity {
        Some(identity) => Ok(User::find_by_id(identity.user_id).one(db).await?),
        None => Ok(None),
    }
}

async fn available_username(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
    username: &str,
) -> Result<String, OidcError> {
    for candidate in [username.to_string(), format!("{username}@{}", provider.slug)] {
        let taken = User::find()
            .filter(user::Column::Name.eq(&candidate))
            .one(db)
            .await?
            .is_some();
        if !taken {
            return Ok(candidate);
        }
    }
    Err(OidcError::AccessDenied("username_conflict"))
}

async fn usable_email(
    db: &DatabaseConnection,
    email: Option<&String>,
) -> Result<Option<String>, OidcError> {
    let Some(email) = email else {
        return Ok(None);
    };
    let taken = User::find()
        .filter(user::Column::Email.eq(email))
        .one(db)
        .await?
        .is_some();
    Ok((!taken).then(|| email.clone()))
}

async fn create_user(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
    identity: &ExternalIdentity,
) -> Result<user::Model, OidcError> {
    let name = available_username(db, provider, &identity.username).await?;
    let email = usable_email(db, identity.email.as_ref()).await?;
    let txn = db.begin().await?;
    let created = user::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name),
        password: Set(String::new()),
        salt: Set(String::new()),
        email: Set(email),
        gravatar_email: Set(None),
        two_factor_secret: Set(None),
        two_factor_enabled: Set(false),
        force_password_change: Set(false),
        auth_source: Set(AUTH_SOURCE.to_string()),
    }
    .insert(&txn)
    .await?;
    user_identities::ActiveModel {
        id: Set(Uuid::new_v4()),
        provider_id: Set(provider.id),
        user_id: Set(created.id),
        subject: Set(identity.subject.clone()),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    info!(user_id = %created.id, provider = %provider.slug, "provisioned user from identity provider");
    Ok(created)
}

pub async fn sign_in(
    db: &DatabaseConnection,
    provider: &identity_providers::Model,
    identity: &ExternalIdentity,
) -> Result<user::Model, OidcError> {
    let role_id = resolve_role(db, provider, &identity.groups).await?;
    let user = match existing_user(db, provider, &identity.subject).await? {
        Some(user) => user,
        None if provider.auto_provision => create_user(db, provider, identity).await?,
        None => return Err(OidcError::AccessDenied("not_provisioned")),
    };
    RbacService::new(db.clone())
        .assign_role_to_user(user.id, provider.organization_id, role_id)
        .await
        .map_err(|error| OidcError::RoleAssignment(error.to_string()))?;
    Ok(user)
}
