pub use sea_orm_migration::prelude::*;

mod m20251026_150747_key;
mod m20251026_150819_invalid_jwt;
mod m20251026_150828_user;
mod m20251026_151103_config;
mod m20251030_192944_expenses;
mod m20251101_200000_subscriptions;
mod m20251215_175420_add_user_2fa_fields;
mod m20251215_180000_add_rbac_tables;
mod m20251216_190000_add_agents_and_metrics;
mod m20251228_120000_add_resource_groups;
mod m20251228_140000_add_docker_resources;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251026_150747_key::Migration),
            Box::new(m20251026_150819_invalid_jwt::Migration),
            Box::new(m20251026_150828_user::Migration),
            Box::new(m20251026_151103_config::Migration),
            Box::new(m20251030_192944_expenses::Migration),
            Box::new(m20251101_200000_subscriptions::Migration),
            Box::new(m20251215_175420_add_user_2fa_fields::Migration),
            Box::new(m20251215_180000_add_rbac_tables::Migration),
            Box::new(m20251216_190000_add_agents_and_metrics::Migration),
            Box::new(m20251228_120000_add_resource_groups::Migration),
            Box::new(m20251228_140000_add_docker_resources::Migration),
        ]
    }
}
