use crate::ceph::core::{CephClient, CephConfig};
use crate::ceph::storage::types::CephPool;
use crate::ceph::storage::{PoolManager, RbdManager};
use anyhow::Result;

pub struct CephManager {
    pub client: CephClient,
    pub pool_manager: PoolManager,
    pub rbd_manager: RbdManager,
}

/// Initialisiert Ceph-Cluster und erstellt Standard-Pools
pub async fn init_ceph() -> Result<CephManager> {
    crate::log_info!("ceph_init", "Initializing Ceph storage system");

    // Konfiguration laden
    let config = CephConfig::from_env()?;
    crate::log_debug!(
        "ceph_init",
        &format!("Using {} monitor hosts", config.mon_hosts.len())
    );

    // Client erstellen
    let client = CephClient::new(config.clone());

    // Auf Cluster warten (max 30 Versuche = 2.5 Minuten)
    client.wait_for_cluster(30).await?;

    // Health-Status prüfen
    let health = client.health_status().await?;
    crate::log_info!(
        "ceph_init",
        &format!("Ceph cluster health: {:?}", health.status)
    );
    crate::log_info!(
        "ceph_init",
        &format!(
            "Monitors: {}, OSDs: {}",
            health.mons.len(),
            health.osds.len()
        )
    );

    // Pool Manager erstellen
    let pool_manager = PoolManager::new(client.clone());

    // Standard-Pools erstellen
    let pools = vec![
        CephPool {
            name: config.default_pool.clone(),
            pg_num: config.default_pg_num,
            pgp_num: config.default_pg_num,
            size: config.default_replication,
            min_size: 2,
        },
        CephPool {
            name: "csfx-postgres".to_string(),
            pg_num: 64,
            pgp_num: 64,
            size: config.default_replication,
            min_size: 2,
        },
        CephPool {
            name: "csfx-metadata".to_string(),
            pg_num: 32,
            pgp_num: 32,
            size: config.default_replication,
            min_size: 2,
        },
    ];

    for pool in pools {
        crate::log_debug!(
            "ceph_init",
            &format!("Ensuring pool '{}' exists", pool.name)
        );
        if let Err(e) = pool_manager.ensure_pool(&pool).await {
            crate::log_warn!(
                "ceph_init",
                &format!("Failed to create pool '{}': {}", pool.name, e)
            );
        }
    }

    // RBD Manager erstellen
    let rbd_manager = RbdManager::new(client.clone());

    crate::log_info!("ceph_init", "Ceph storage system initialized successfully");

    Ok(CephManager {
        client,
        pool_manager,
        rbd_manager,
    })
}

/// Erstellt PostgreSQL Volumes auf dem Ceph-Cluster
pub async fn create_postgres_volumes(ceph: &CephManager, node_count: u32) -> Result<Vec<String>> {
    crate::log_info!(
        "ceph_init",
        &format!("Creating PostgreSQL volumes for {} nodes", node_count)
    );

    let mut volumes = Vec::new();

    for i in 1..=node_count {
        let volume_name = format!("postgres-node-{}", i);

        let volume = crate::ceph::storage::types::CephVolume {
            name: volume_name.clone(),
            pool: "csfx-postgres".to_string(),
            size_mb: 10240, // 10 GB
            features: vec!["layering".to_string(), "exclusive-lock".to_string()],
            encrypted: false,
        };

        // Erstelle Volume falls nicht vorhanden
        if !ceph
            .rbd_manager
            .image_exists(&volume.pool, &volume.name)
            .await?
        {
            ceph.rbd_manager.create_image(&volume).await?;
            volumes.push(volume_name);
        } else {
            crate::log_info!(
                "ceph_init",
                &format!("Volume '{}' already exists", volume_name)
            );
            volumes.push(volume_name);
        }
    }

    crate::log_info!(
        "ceph_init",
        &format!("Created {} PostgreSQL volumes", volumes.len())
    );

    Ok(volumes)
}
