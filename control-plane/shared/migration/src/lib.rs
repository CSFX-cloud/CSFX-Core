pub use sea_orm_migration::prelude::*;

mod m20251026_150747_key;
mod m20251026_150819_invalid_jwt;
mod m20251026_150828_user;
mod m20251026_151103_config;
mod m20251215_175420_add_user_2fa_fields;
mod m20251215_180000_add_rbac_tables;
mod m20251216_190000_add_agents_and_metrics;
mod m20260214_100000_add_registry_tables;
mod m20260303_000000_registry_security;
mod m20260304_000000_pki_certificates;
mod m20260304_120000_drop_api_key_column;
mod m20260305_000000_add_workloads;
mod m20260306_000000_add_volumes;
mod m20260306_120000_add_failover_events;
mod m20260307_000000_add_networks;
mod m20260308_000000_add_org_scoping;
mod m20260309_000000_add_bootstrap_tokens;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251026_150747_key::Migration),
            Box::new(m20251026_150819_invalid_jwt::Migration),
            Box::new(m20251026_150828_user::Migration),
            Box::new(m20251026_151103_config::Migration),
            Box::new(m20251215_175420_add_user_2fa_fields::Migration),
            Box::new(m20251215_180000_add_rbac_tables::Migration),
            Box::new(m20251216_190000_add_agents_and_metrics::Migration),
            Box::new(m20260214_100000_add_registry_tables::Migration),
            Box::new(m20260303_000000_registry_security::Migration),
            Box::new(m20260304_000000_pki_certificates::Migration),
            Box::new(m20260304_120000_drop_api_key_column::Migration),
            Box::new(m20260305_000000_add_workloads::Migration),
            Box::new(m20260306_000000_add_volumes::Migration),
            Box::new(m20260306_120000_add_failover_events::Migration),
            Box::new(m20260307_000000_add_networks::Migration),
            Box::new(m20260308_000000_add_org_scoping::Migration),
            Box::new(m20260309_000000_add_bootstrap_tokens::Migration),
        ]
    }
}
