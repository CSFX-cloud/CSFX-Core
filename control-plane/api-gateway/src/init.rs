use entity::{
    key, organization, permission, role, role_permission, user, user_organization, Key,
    Organization, Permission, Role, RolePermission, User, UserOrganization,
};
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use uuid::Uuid;

use crate::auth::crypto::{generate_salt, hash_password, RsaKeyPair};

pub async fn initialize_database(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Initializing database with default data...");

    // 1. Create RSA key pair if not exists
    let key_exists = Key::find()
        .filter(key::Column::Name.eq("main"))
        .one(db)
        .await?
        .is_some();

    if !key_exists {
        tracing::info!("Creating RSA key pair...");
        let key_pair = RsaKeyPair::generate()?;
        let key_id = Uuid::new_v4();
        let new_key = key::ActiveModel {
            id: ActiveValue::Set(key_id),
            name: ActiveValue::Set("main".to_string()),
            private_key: ActiveValue::Set(key_pair.private_key),
        };
        Key::insert(new_key).exec_without_returning(db).await?;
        tracing::info!("RSA key pair created successfully");
    } else {
        tracing::info!("RSA key pair already exists");
    }

    // 2. Create default organization
    let default_org_exists = Organization::find()
        .filter(organization::Column::Name.eq("Default Organization"))
        .one(db)
        .await?;

    let default_org_id = if let Some(org) = default_org_exists {
        tracing::info!("Default organization already exists");
        org.id
    } else {
        tracing::info!("Creating default organization...");
        let org_id = Uuid::new_v4();
        let now = chrono::Utc::now().naive_utc();
        let new_org = organization::ActiveModel {
            id: ActiveValue::Set(org_id),
            name: ActiveValue::Set("Default Organization".to_string()),
            description: ActiveValue::Set(Some("Default organization for all users".to_string())),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };
        Organization::insert(new_org)
            .exec_without_returning(db)
            .await?;
        tracing::info!("Default organization created");
        org_id
    };

    // 3. Create default permissions
    let permissions_to_create = vec![
        ("organization.view", "organization", "view", "View organization details"),
        ("organization.update", "organization", "update", "Update organization"),
        ("organization.delete", "organization", "delete", "Delete organization"),
        ("members.view", "members", "view", "View organization members"),
        ("members.create", "members", "create", "Add members to organization"),
        ("members.update", "members", "update", "Update member roles"),
        ("members.delete", "members", "delete", "Remove members from organization"),
        ("users.view", "users", "view", "View user list and details"),
        ("users.create", "users", "create", "Create new users"),
        ("users.update", "users", "update", "Update user details"),
        ("users.delete", "users", "delete", "Delete users"),
        ("agents.view", "agents", "view", "View agents and metrics"),
        ("agents.manage", "agents", "manage", "Register and manage agents"),
        ("workloads.view", "workloads", "view", "View workloads"),
        ("workloads.manage", "workloads", "manage", "Create and delete workloads"),
        ("volumes.view", "volumes", "view", "View volumes and snapshots"),
        ("volumes.manage", "volumes", "manage", "Create, attach, detach and delete volumes"),
        ("networks.view", "networks", "view", "View networks and policies"),
        ("networks.manage", "networks", "manage", "Create and manage networks"),
    ];

    let mut permission_map = std::collections::HashMap::new();

    for (name, resource, action, description) in permissions_to_create {
        let perm_exists = Permission::find()
            .filter(permission::Column::Name.eq(name))
            .one(db)
            .await?;

        let perm_id = if let Some(perm) = perm_exists {
            perm.id
        } else {
            let perm_id = Uuid::new_v4();
            let new_perm = permission::ActiveModel {
                id: ActiveValue::Set(perm_id),
                name: ActiveValue::Set(name.to_string()),
                resource: ActiveValue::Set(resource.to_string()),
                action: ActiveValue::Set(action.to_string()),
                description: ActiveValue::Set(Some(description.to_string())),
            };
            Permission::insert(new_perm)
                .exec_without_returning(db)
                .await?;
            perm_id
        };

        permission_map.insert(name, perm_id);
    }

    // 4. Create default roles
    let admin_role_exists = Role::find()
        .filter(role::Column::Name.eq("Admin"))
        .filter(role::Column::OrganizationId.eq(default_org_id))
        .one(db)
        .await?;

    let admin_role_id = if let Some(role) = admin_role_exists {
        tracing::info!("Admin role already exists");
        role.id
    } else {
        tracing::info!("Creating Admin role...");
        let role_id = Uuid::new_v4();
        let now = chrono::Utc::now().naive_utc();
        let new_role = role::ActiveModel {
            id: ActiveValue::Set(role_id),
            name: ActiveValue::Set("Admin".to_string()),
            description: ActiveValue::Set(Some("Full access to organization".to_string())),
            organization_id: ActiveValue::Set(default_org_id),
            is_system_role: ActiveValue::Set(true),
            created_at: ActiveValue::Set(now),
        };
        Role::insert(new_role).exec_without_returning(db).await?;

        // Assign all permissions to Admin role
        for perm_id in permission_map.values() {
            let role_perm = role_permission::ActiveModel {
                role_id: ActiveValue::Set(role_id),
                permission_id: ActiveValue::Set(*perm_id),
            };
            RolePermission::insert(role_perm)
                .exec_without_returning(db)
                .await?;
        }

        tracing::info!("Admin role created with all permissions");
        role_id
    };

    // 4b. Create Operator role
    let operator_role_exists = Role::find()
        .filter(role::Column::Name.eq("Operator"))
        .filter(role::Column::OrganizationId.eq(default_org_id))
        .one(db)
        .await?;

    if operator_role_exists.is_none() {
        tracing::info!("Creating Operator role...");
        let role_id = Uuid::new_v4();
        let now = chrono::Utc::now().naive_utc();
        let new_role = role::ActiveModel {
            id: ActiveValue::Set(role_id),
            name: ActiveValue::Set("Operator".to_string()),
            description: ActiveValue::Set(Some("Can manage resources but not users".to_string())),
            organization_id: ActiveValue::Set(default_org_id),
            is_system_role: ActiveValue::Set(true),
            created_at: ActiveValue::Set(now),
        };
        Role::insert(new_role).exec_without_returning(db).await?;

        let operator_perms = [
            "agents.view", "agents.manage",
            "workloads.view", "workloads.manage",
            "volumes.view", "volumes.manage",
            "networks.view", "networks.manage",
            "members.view",
        ];
        for perm_name in operator_perms {
            if let Some(perm_id) = permission_map.get(perm_name) {
                let role_perm = role_permission::ActiveModel {
                    role_id: ActiveValue::Set(role_id),
                    permission_id: ActiveValue::Set(*perm_id),
                };
                RolePermission::insert(role_perm).exec_without_returning(db).await?;
            }
        }
        tracing::info!("Operator role created");
    }

    // 4c. Create Viewer role
    let viewer_role_exists = Role::find()
        .filter(role::Column::Name.eq("Viewer"))
        .filter(role::Column::OrganizationId.eq(default_org_id))
        .one(db)
        .await?;

    if viewer_role_exists.is_none() {
        tracing::info!("Creating Viewer role...");
        let role_id = Uuid::new_v4();
        let now = chrono::Utc::now().naive_utc();
        let new_role = role::ActiveModel {
            id: ActiveValue::Set(role_id),
            name: ActiveValue::Set("Viewer".to_string()),
            description: ActiveValue::Set(Some("Read-only access to resources".to_string())),
            organization_id: ActiveValue::Set(default_org_id),
            is_system_role: ActiveValue::Set(true),
            created_at: ActiveValue::Set(now),
        };
        Role::insert(new_role).exec_without_returning(db).await?;

        let viewer_perms = [
            "agents.view", "workloads.view", "volumes.view", "networks.view",
            "organization.view", "members.view",
        ];
        for perm_name in viewer_perms {
            if let Some(perm_id) = permission_map.get(perm_name) {
                let role_perm = role_permission::ActiveModel {
                    role_id: ActiveValue::Set(role_id),
                    permission_id: ActiveValue::Set(*perm_id),
                };
                RolePermission::insert(role_perm).exec_without_returning(db).await?;
            }
        }
        tracing::info!("Viewer role created");
    }

    // 5. Create admin user if not exists
    let admin_exists = User::find()
        .filter(user::Column::Name.eq("admin@local.com"))
        .one(db)
        .await?;

    let admin_user_id = if let Some(existing_admin) = admin_exists {
        tracing::info!("Admin user already exists");
        existing_admin.id
    } else {
        tracing::info!("Creating default admin user...");
        let salt = generate_salt();
        let hashed_password = hash_password("admin", &salt)?;
        let user_id = Uuid::new_v4();

        let admin_user = user::ActiveModel {
            id: ActiveValue::Set(user_id),
            name: ActiveValue::Set("admin@local.com".to_string()),
            password: ActiveValue::Set(hashed_password),
            salt: ActiveValue::Set(salt),
            email: ActiveValue::Set(Some("admin@local.com".to_string())),
            two_factor_secret: ActiveValue::NotSet,
            two_factor_enabled: ActiveValue::Set(false),
            force_password_change: ActiveValue::Set(true),
        };

        User::insert(admin_user).exec_without_returning(db).await?;
        tracing::info!("Default admin user created: admin@local.com / admin - Password change required on first login");
        user_id
    };

    // Ensure admin user is in the default organization with Admin role
    let admin_org_exists = UserOrganization::find()
        .filter(user_organization::Column::UserId.eq(admin_user_id))
        .filter(user_organization::Column::OrganizationId.eq(default_org_id))
        .one(db)
        .await?;

    if admin_org_exists.is_none() {
        tracing::info!("Assigning admin user to default organization with Admin role...");
        let user_org = user_organization::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            user_id: ActiveValue::Set(admin_user_id),
            organization_id: ActiveValue::Set(default_org_id),
            role_id: ActiveValue::Set(admin_role_id),
            joined_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        };
        UserOrganization::insert(user_org)
            .exec_without_returning(db)
            .await?;
        tracing::info!("Admin user assigned to default organization");
    }

    tracing::info!("Database initialization completed");
    Ok(())
}
