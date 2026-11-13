use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{NaiveDateTime, Utc};
use entity::entities::volumes;
use sea_orm::DatabaseConnection;
use shared::leader::LeaderLease;
use uuid::Uuid;

use crate::ceph::ops::CephManager;
use crate::db::volumes as db;
use crate::models::volume::VolumeStatus;
use crate::services::volume::VolumeService;
use crate::{log_error, log_info, log_warn};

const RECONCILE_INTERVAL: Duration = Duration::from_secs(60);
const GRACE: Duration = Duration::from_secs(600);

type ImageKey = (String, String);

#[derive(Debug, PartialEq)]
pub struct RowView {
    pub id: Uuid,
    pub pool: String,
    pub image_name: String,
    pub status: VolumeStatus,
    pub touched_at: NaiveDateTime,
}

#[derive(Debug, PartialEq)]
pub enum Repair {
    Activate(Uuid),
    Fail(Uuid),
    FinishDelete(Uuid),
}

impl From<&volumes::Model> for RowView {
    fn from(model: &volumes::Model) -> Self {
        Self {
            id: model.id,
            pool: model.pool.clone(),
            image_name: model.image_name.clone(),
            status: VolumeStatus::from_str(&model.status),
            touched_at: model.updated_at.unwrap_or(model.created_at),
        }
    }
}

pub fn stuck_repairs(
    rows: &[RowView],
    images: &HashSet<ImageKey>,
    now: NaiveDateTime,
    grace: Duration,
) -> Vec<Repair> {
    rows.iter()
        .filter(|row| {
            (now - row.touched_at)
                .to_std()
                .is_ok_and(|age| age >= grace)
        })
        .filter_map(|row| match row.status {
            VolumeStatus::Creating => {
                let present = images.contains(&(row.pool.clone(), row.image_name.clone()));
                Some(if present {
                    Repair::Activate(row.id)
                } else {
                    Repair::Fail(row.id)
                })
            }
            VolumeStatus::Deleting => Some(Repair::FinishDelete(row.id)),
            _ => None,
        })
        .collect()
}

pub fn orphan_candidates(rows: &[RowView], images: &HashSet<ImageKey>) -> HashSet<ImageKey> {
    let known: HashSet<(&str, &str)> = rows
        .iter()
        .map(|row| (row.pool.as_str(), row.image_name.as_str()))
        .collect();
    images
        .iter()
        .filter(|(pool, name)| {
            db::is_managed_image_name(name) && !known.contains(&(pool.as_str(), name.as_str()))
        })
        .cloned()
        .collect()
}

#[derive(Default)]
pub struct OrphanTracker {
    first_seen: HashMap<ImageKey, Instant>,
}

impl OrphanTracker {
    pub fn confirmed(
        &mut self,
        candidates: HashSet<ImageKey>,
        now: Instant,
        grace: Duration,
    ) -> Vec<ImageKey> {
        self.first_seen.retain(|key, _| candidates.contains(key));
        for key in candidates {
            self.first_seen.entry(key).or_insert(now);
        }
        self.first_seen
            .iter()
            .filter(|(_, seen)| now.duration_since(**seen) >= grace)
            .map(|(key, _)| key.clone())
            .collect()
    }
}

struct Reconciler {
    db: DatabaseConnection,
    ceph: Arc<CephManager>,
    service: Arc<VolumeService>,
    tracker: OrphanTracker,
}

impl Reconciler {
    async fn run_once(&mut self) -> anyhow::Result<()> {
        let models = db::list_models(&self.db).await?;
        let rows: Vec<RowView> = models.iter().map(RowView::from).collect();
        let images = self.list_pool_images(&rows).await?;

        for repair in stuck_repairs(&rows, &images, Utc::now().naive_utc(), GRACE) {
            self.apply(repair, &models).await;
        }

        let candidates = orphan_candidates(&rows, &images);
        for (pool, image) in self.tracker.confirmed(candidates, Instant::now(), GRACE) {
            self.remove_orphan(&pool, &image).await;
        }
        Ok(())
    }

    async fn list_pool_images(&self, rows: &[RowView]) -> anyhow::Result<HashSet<ImageKey>> {
        let mut pools: HashSet<&str> = rows.iter().map(|row| row.pool.as_str()).collect();
        pools.insert(db::DEFAULT_POOL);

        let mut images = HashSet::new();
        for pool in pools {
            for image in self.ceph.rbd_manager.list_images(pool).await? {
                images.insert((pool.to_string(), image));
            }
        }
        Ok(images)
    }

    async fn apply(&self, repair: Repair, models: &[volumes::Model]) {
        let (id, outcome) = match repair {
            Repair::Activate(id) => (id, self.set_status(id, VolumeStatus::Available).await),
            Repair::Fail(id) => (id, self.set_status(id, VolumeStatus::Error).await),
            Repair::FinishDelete(id) => match models.iter().find(|model| model.id == id) {
                Some(model) => (id, self.service.finish_delete(model).await),
                None => return,
            },
        };
        match outcome {
            Ok(()) => log_info!("reconcile", &format!("Repaired volume volume_id={}", id)),
            Err(e) => log_error!(
                "reconcile",
                &format!("Repair failed volume_id={} err={}", id, e)
            ),
        }
    }

    async fn set_status(&self, id: Uuid, status: VolumeStatus) -> Result<(), String> {
        db::set_status(&self.db, id, status)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn remove_orphan(&self, pool: &str, image: &str) {
        let rbd = &self.ceph.rbd_manager;
        let removed = match rbd.purge_snapshots(pool, image).await {
            Ok(()) => rbd.delete_image(pool, image).await,
            Err(e) => Err(e),
        };
        match removed {
            Ok(()) => log_warn!(
                "reconcile",
                &format!("Removed orphan image pool={} image={}", pool, image)
            ),
            Err(e) => log_error!(
                "reconcile",
                &format!(
                    "Orphan removal failed pool={} image={} err={:#}",
                    pool, image, e
                )
            ),
        }
    }
}

pub async fn run(
    db: DatabaseConnection,
    ceph: Arc<CephManager>,
    service: Arc<VolumeService>,
    leader: LeaderLease,
) {
    let mut reconciler = Reconciler {
        db,
        ceph,
        service,
        tracker: OrphanTracker::default(),
    };
    let mut tick = tokio::time::interval(RECONCILE_INTERVAL);
    loop {
        tick.tick().await;
        if !leader.is_leader() {
            reconciler.tracker = OrphanTracker::default();
            continue;
        }
        if let Err(e) = reconciler.run_once().await {
            log_error!("reconcile", &format!("Reconcile pass failed err={:#}", e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: VolumeStatus, age_secs: i64, image: &str) -> RowView {
        RowView {
            id: Uuid::new_v4(),
            pool: "p".to_string(),
            image_name: image.to_string(),
            status,
            touched_at: Utc::now().naive_utc() - chrono::Duration::seconds(age_secs),
        }
    }

    fn key(image: &str) -> ImageKey {
        ("p".to_string(), image.to_string())
    }

    #[test]
    fn young_rows_are_left_alone() {
        let rows = [row(VolumeStatus::Creating, 5, "a")];
        let repairs = stuck_repairs(&rows, &HashSet::new(), Utc::now().naive_utc(), GRACE);
        assert!(repairs.is_empty());
    }

    #[test]
    fn stuck_creating_follows_image_presence() {
        let rows = [
            row(VolumeStatus::Creating, 3600, "a"),
            row(VolumeStatus::Creating, 3600, "b"),
        ];
        let images = HashSet::from([key("a")]);
        let repairs = stuck_repairs(&rows, &images, Utc::now().naive_utc(), GRACE);
        assert_eq!(
            repairs,
            vec![Repair::Activate(rows[0].id), Repair::Fail(rows[1].id)]
        );
    }

    #[test]
    fn stuck_deleting_is_finished_and_settled_rows_ignored() {
        let rows = [
            row(VolumeStatus::Deleting, 3600, "a"),
            row(VolumeStatus::Available, 3600, "b"),
            row(VolumeStatus::Error, 3600, "c"),
        ];
        let repairs = stuck_repairs(&rows, &HashSet::new(), Utc::now().naive_utc(), GRACE);
        assert_eq!(repairs, vec![Repair::FinishDelete(rows[0].id)]);
    }

    #[test]
    fn only_managed_unknown_images_are_orphans() {
        let managed = db::generate_image_name("vol");
        let known = db::generate_image_name("vol");
        let rows = [row(VolumeStatus::Available, 0, &known)];
        let images = HashSet::from([key(&managed), key(&known), key("foreign-image")]);
        assert_eq!(
            orphan_candidates(&rows, &images),
            HashSet::from([key(&managed)])
        );
    }

    #[test]
    fn orphans_are_confirmed_only_after_grace() {
        let mut tracker = OrphanTracker::default();
        let start = Instant::now();
        let candidates = HashSet::from([key("x")]);
        assert!(tracker
            .confirmed(candidates.clone(), start, GRACE)
            .is_empty());
        let later = start + GRACE;
        assert_eq!(tracker.confirmed(candidates, later, GRACE), vec![key("x")]);
        assert!(tracker.confirmed(HashSet::new(), later, GRACE).is_empty());
        assert!(tracker
            .confirmed(HashSet::from([key("x")]), later, GRACE)
            .is_empty());
    }
}
