use entity::{
    organization, permission, role, role_permission, user_organization, Organization, Permission,
    Role, RolePermission, UserOrganization,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RbacService {
    db: DatabaseConnection,
}

impl RbacService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Check if a user has a specific permission in an organization
    pub async fn has_permission(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        resource: &str,
        action: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Get user's role in the organization
        let user_org = UserOrganization::find()
            .filter(user_organization::Column::UserId.eq(user_id))
            .filter(user_organization::Column::OrganizationId.eq(organization_id))
            .one(&self.db)
            .await?;

        let role_id = match user_org {
            Some(uo) => uo.role_id,
            None => return Ok(false), // User not in organization
        };

        // Get role's permissions
        let permissions = self.get_role_permissions(role_id).await?;

        // Check if permission exists
        Ok(permissions
            .iter()
            .any(|p| p.resource == resource && (p.action == action || p.action == "*")))
    }

    /// Get all permissions for a role
    pub async fn get_role_permissions(
        &self,
        role_id: Uuid,
    ) -> Result<Vec<permission::Model>, Box<dyn std::error::Error>> {
        let role_perms = RolePermission::find()
            .filter(role_permission::Column::RoleId.eq(role_id))
            .all(&self.db)
            .await?;

        let permission_ids: Vec<Uuid> = role_perms.iter().map(|rp| rp.permission_id).collect();

        let permissions = Permission::find()
            .filter(permission::Column::Id.is_in(permission_ids))
            .all(&self.db)
            .await?;

        Ok(permissions)
    }

    /// Get user's organizations
    #[allow(dead_code)]
    pub async fn get_user_organizations(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<organization::Model>, Box<dyn std::error::Error>> {
        let user_orgs = UserOrganization::find()
            .filter(user_organization::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;

        let org_ids: Vec<Uuid> = user_orgs.iter().map(|uo| uo.organization_id).collect();

        let organizations = Organization::find()
            .filter(organization::Column::Id.is_in(org_ids))
            .all(&self.db)
            .await?;

        Ok(organizations)
    }

    /// Get user's role in an organization
    pub async fn get_user_role(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> Result<Option<role::Model>, Box<dyn std::error::Error>> {
        let user_org = UserOrganization::find()
            .filter(user_organization::Column::UserId.eq(user_id))
            .filter(user_organization::Column::OrganizationId.eq(organization_id))
            .one(&self.db)
            .await?;

        if let Some(uo) = user_org {
            let role = Role::find_by_id(uo.role_id).one(&self.db).await?;
            Ok(role)
        } else {
            Ok(None)
        }
    }

    /// Assign a role to a user in an organization
    pub async fn assign_role_to_user(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        role_id: Uuid,
    ) -> Result<user_organization::Model, Box<dyn std::error::Error>> {
        // Check if user is already in organization
        let existing = UserOrganization::find()
            .filter(user_organization::Column::UserId.eq(user_id))
            .filter(user_organization::Column::OrganizationId.eq(organization_id))
            .one(&self.db)
            .await?;

        if let Some(existing) = existing {
            // Update existing role
            let mut active: user_organization::ActiveModel = existing.clone().into();
            active.role_id = ActiveValue::Set(role_id);
            let updated = active.update(&self.db).await?;
            Ok(updated)
        } else {
            // Create new user-organization relationship
            let new_user_org = user_organization::ActiveModel {
                id: ActiveValue::Set(Uuid::new_v4()),
                user_id: ActiveValue::Set(user_id),
                organization_id: ActiveValue::Set(organization_id),
                role_id: ActiveValue::Set(role_id),
                joined_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            };

            let result = new_user_org.insert(&self.db).await?;
            Ok(result)
        }
    }

    /// Remove user from organization
    pub async fn remove_user_from_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        UserOrganization::delete_many()
            .filter(user_organization::Column::UserId.eq(user_id))
            .filter(user_organization::Column::OrganizationId.eq(organization_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    /// Create a new role in an organization
    #[allow(dead_code)]
    pub async fn create_role(
        &self,
        organization_id: Uuid,
        name: String,
        description: Option<String>,
        is_system_role: bool,
    ) -> Result<role::Model, Box<dyn std::error::Error>> {
        let new_role = role::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            name: ActiveValue::Set(name),
            description: ActiveValue::Set(description),
            organization_id: ActiveValue::Set(organization_id),
            is_system_role: ActiveValue::Set(is_system_role),
            created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        };

        let result = new_role.insert(&self.db).await?;
        Ok(result)
    }

    /// Assign permissions to a role
    #[allow(dead_code)]
    pub async fn assign_permissions_to_role(
        &self,
        role_id: Uuid,
        permission_ids: Vec<Uuid>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Delete existing permissions
        RolePermission::delete_many()
            .filter(role_permission::Column::RoleId.eq(role_id))
            .exec(&self.db)
            .await?;

        // Add new permissions
        for permission_id in permission_ids {
            let role_perm = role_permission::ActiveModel {
                role_id: ActiveValue::Set(role_id),
                permission_id: ActiveValue::Set(permission_id),
            };
            role_perm.insert(&self.db).await?;
        }

        Ok(())
    }

    /// Get organization members with their roles
    pub async fn get_organization_members(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<(user_organization::Model, role::Model)>, Box<dyn std::error::Error>> {
        let members = UserOrganization::find()
            .filter(user_organization::Column::OrganizationId.eq(organization_id))
            .find_also_related(Role)
            .all(&self.db)
            .await?;

        let result = members
            .into_iter()
            .filter_map(|(uo, role)| role.map(|r| (uo, r)))
            .collect();

        Ok(result)
    }
}
