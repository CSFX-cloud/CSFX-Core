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
mod m20260523_000000_add_ssh_keys;
mod m20260625_000000_add_resource_groups;
mod m20260628_000000_add_volume_mounts;
mod m20260628_100000_rg_cidr_unique_agent_wg;
mod m20260701_000000_add_logs_and_settings;
mod m20260702_000000_add_agent_wg_tunnel_ip;
mod m20260704_000000_add_workload_stacks;
mod m20260710_000000_add_workload_restart_policy;
mod m20260711_000000_add_runtime_class;
mod m20260711_010000_add_agent_cordoned;
mod m20260712_000000_add_workload_lifecycle;
mod m20260712_010000_add_resource_group_vpn_peers;
mod m20260723_000000_add_resource_group_appearance;
mod m20260723_010000_runtime_class_default_firecracker;
mod m20260809_000000_add_user_gravatar_email;
mod m20260816_000000_add_object_storage;
mod m20260816_010000_add_bucket_master_key;
mod m20260816_020000_garage_nodes_agent_id_nullable;
mod m20260918_000000_add_alerts_and_maintenance;

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
            Box::new(m20260523_000000_add_ssh_keys::Migration),
            Box::new(m20260625_000000_add_resource_groups::Migration),
            Box::new(m20260628_000000_add_volume_mounts::Migration),
            Box::new(m20260628_100000_rg_cidr_unique_agent_wg::Migration),
            Box::new(m20260701_000000_add_logs_and_settings::Migration),
            Box::new(m20260702_000000_add_agent_wg_tunnel_ip::Migration),
            Box::new(m20260704_000000_add_workload_stacks::Migration),
            Box::new(m20260710_000000_add_workload_restart_policy::Migration),
            Box::new(m20260711_000000_add_runtime_class::Migration),
            Box::new(m20260711_010000_add_agent_cordoned::Migration),
            Box::new(m20260712_000000_add_workload_lifecycle::Migration),
            Box::new(m20260712_010000_add_resource_group_vpn_peers::Migration),
            Box::new(m20260723_000000_add_resource_group_appearance::Migration),
            Box::new(m20260723_010000_runtime_class_default_firecracker::Migration),
            Box::new(m20260809_000000_add_user_gravatar_email::Migration),
            Box::new(m20260816_000000_add_object_storage::Migration),
            Box::new(m20260816_010000_add_bucket_master_key::Migration),
            Box::new(m20260816_020000_garage_nodes_agent_id_nullable::Migration),
            Box::new(m20260918_000000_add_alerts_and_maintenance::Migration),
        ]
    }
}
