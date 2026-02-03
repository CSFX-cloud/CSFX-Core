use entity::{
    key, organization, permission, role, role_permission, user, user_organization, Key,
    Organization, Permission, Role, RolePermission, User, UserOrganization,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
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
        (
            "organization.view",
            "organization",
            "view",
            "View organization details",
        ),
        (
            "organization.update",
            "organization",
            "update",
            "Update organization",
        ),
        (
            "organization.delete",
            "organization",
            "delete",
            "Delete organization",
        ),
        (
            "members.view",
            "members",
            "view",
            "View organization members",
        ),
        (
            "members.create",
            "members",
            "create",
            "Add members to organization",
        ),
        ("members.update", "members", "update", "Update member roles"),
        (
            "members.delete",
            "members",
            "delete",
            "Remove members from organization",
        ),
        ("users.view", "users", "view", "View user list and details"),
        ("users.create", "users", "create", "Create new users"),
        ("users.update", "users", "update", "Update user details"),
        ("users.delete", "users", "delete", "Delete users"),
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

    // 5. Create admin user if not exists
    let admin_exists = User::find()
        .filter(user::Column::Name.eq("admin@local.com"))
        .one(db)
        .await?;

    if admin_exists.is_none() {
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

        // Add admin user to default organization with Admin role
        let user_org_id = Uuid::new_v4();
        let now = chrono::Utc::now().naive_utc();
        let user_org = user_organization::ActiveModel {
            id: ActiveValue::Set(user_org_id),
            user_id: ActiveValue::Set(user_id),
            organization_id: ActiveValue::Set(default_org_id),
            role_id: ActiveValue::Set(admin_role_id),
            joined_at: ActiveValue::Set(now),
        };
        UserOrganization::insert(user_org)
            .exec_without_returning(db)
            .await?;

        tracing::info!("Default admin user created: admin@local.com / admin - Password change required on first login");
    } else {
        tracing::info!("Admin user already exists");
    }

    // 6. Initialize marketplace templates
    initialize_marketplace_templates(db).await?;

    tracing::info!("Database initialization completed");
    Ok(())
}

async fn initialize_marketplace_templates(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    use entity::{marketplace_templates, MarketplaceTemplates};

    // Check if templates already exist
    let template_count = MarketplaceTemplates::find().count(db).await?;

    if template_count > 0 {
        tracing::info!(
            "Marketplace templates already initialized ({} templates)",
            template_count
        );
        return Ok(());
    }

    tracing::info!("Initializing marketplace templates...");

    let default_templates = vec![
        // Docker Container
        (
            "docker-container",
            "Docker Container",
            "Leerer Docker Container. Wähle dein eigenes Image und konfiguriere es nach deinen Wünschen.",
            "🐳",
            "base",
            "docker-container",
            serde_json::json!({
                "image": "nginx:alpine",
                "ports": [{"container": 80, "host": 8080}],
                "environment": {},
                "volumes": [],
                "restart_policy": "unless-stopped"
            }),
            false,
        ),
        // Docker Stack
        (
            "docker-stack",
            "Docker Stack",
            "Leerer Docker Stack. Erstelle einen Multi-Container Stack mit mehreren Services.",
            "📦",
            "base",
            "docker-stack",
            serde_json::json!({
                "services": [
                    {
                        "name": "app",
                        "image": "nginx:alpine",
                        "ports": [{"container": 80, "host": 8080}],
                        "environment": {},
                        "volumes": [],
                        "restart_policy": "unless-stopped"
                    }
                ]
            }),
            false,
        ),
    ];

    let now = chrono::Utc::now().naive_utc();
    let mut created_count = 0;

    for (template_id, name, description, icon, category, resource_type, configuration, popular) in
        default_templates
    {
        let template = marketplace_templates::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            template_id: ActiveValue::Set(template_id.to_string()),
            name: ActiveValue::Set(name.to_string()),
            description: ActiveValue::Set(description.to_string()),
            icon: ActiveValue::Set(icon.to_string()),
            category: ActiveValue::Set(category.to_string()),
            resource_type: ActiveValue::Set(resource_type.to_string()),
            configuration: ActiveValue::Set(configuration),
            popular: ActiveValue::Set(popular),
            install_count: ActiveValue::Set(0),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        if template.insert(db).await.is_ok() {
            created_count += 1;
        }
    }

    tracing::info!("✅ Created {} marketplace templates", created_count);
    Ok(())
}
