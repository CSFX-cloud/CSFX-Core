use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Microvm,
    Vm,
}

impl Kind {
    pub fn of(runtime_class: &str) -> Self {
        match runtime_class {
            "vm" => Self::Vm,
            _ => Self::Microvm,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Scheduled,
    Creating,
    Starting,
    Failed,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Creating => "creating",
            Self::Starting => "starting",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct WorkloadEntry {
    pub kind: Kind,
    pub handle: Option<String>,
    pub phase: Option<Phase>,
    pub restarts: u32,
}

#[derive(Clone, Default)]
pub struct WorkloadTable(Arc<Mutex<HashMap<String, WorkloadEntry>>>);

impl WorkloadTable {
    pub async fn handle(&self, workload_id: &str) -> Option<String> {
        self.0
            .lock()
            .await
            .get(workload_id)
            .and_then(|entry| entry.handle.clone())
    }

    pub async fn running(&self) -> Vec<(String, Kind, String)> {
        self.0
            .lock()
            .await
            .iter()
            .filter_map(|(id, entry)| Some((id.clone(), entry.kind, entry.handle.clone()?)))
            .collect()
    }

    pub async fn pending(&self) -> Vec<(String, Phase)> {
        self.0
            .lock()
            .await
            .iter()
            .filter(|(_, entry)| entry.handle.is_none())
            .filter_map(|(id, entry)| Some((id.clone(), entry.phase?)))
            .collect()
    }

    pub async fn stale(&self, desired: &HashSet<&str>) -> Vec<(String, WorkloadEntry)> {
        self.0
            .lock()
            .await
            .iter()
            .filter(|(id, _)| !desired.contains(id.as_str()))
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect()
    }

    pub async fn remove(&self, workload_id: &str) {
        self.0.lock().await.remove(workload_id);
    }

    pub async fn restarts(&self, workload_id: &str) -> u32 {
        self.0
            .lock()
            .await
            .get(workload_id)
            .map_or(0, |entry| entry.restarts)
    }

    pub async fn record_restart(&self, workload_id: &str, limit: Option<i32>) -> Result<u32, u32> {
        let mut table = self.0.lock().await;
        let entry = table.entry(workload_id.to_string()).or_default();
        if limit.is_some_and(|max| entry.restarts as i32 >= max) {
            return Err(entry.restarts);
        }
        entry.restarts += 1;
        Ok(entry.restarts)
    }

    pub async fn set_phase(&self, workload_id: &str, phase: Phase) {
        self.0
            .lock()
            .await
            .entry(workload_id.to_string())
            .or_default()
            .phase = Some(phase);
    }

    pub async fn clear_phase(&self, workload_id: &str) {
        if let Some(entry) = self.0.lock().await.get_mut(workload_id) {
            entry.phase = None;
        }
    }

    pub async fn clear_handle(&self, workload_id: &str) {
        if let Some(entry) = self.0.lock().await.get_mut(workload_id) {
            entry.handle = None;
        }
    }

    pub async fn started(&self, workload_id: &str, kind: Kind, handle: String) {
        let mut table = self.0.lock().await;
        let entry = table.entry(workload_id.to_string()).or_default();
        entry.kind = kind;
        entry.handle = Some(handle);
        entry.phase = Some(Phase::Starting);
    }

    pub async fn mark_kinds(&self, kinds: impl IntoIterator<Item = (String, Kind)>) {
        let mut table = self.0.lock().await;
        for (workload_id, kind) in kinds {
            table.entry(workload_id).or_default().kind = kind;
        }
    }

    pub async fn adopt(&self, recovered: impl IntoIterator<Item = (String, Kind, String)>) {
        let mut table = self.0.lock().await;
        for (workload_id, kind, handle) in recovered {
            let entry = table.entry(workload_id).or_default();
            entry.kind = kind;
            entry.handle = Some(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn restarts_stop_at_the_limit() {
        let table = WorkloadTable::default();
        assert_eq!(table.record_restart("w", Some(2)).await, Ok(1));
        assert_eq!(table.record_restart("w", Some(2)).await, Ok(2));
        assert_eq!(table.record_restart("w", Some(2)).await, Err(2));
        assert_eq!(table.record_restart("w", None).await, Ok(3));
        assert_eq!(table.restarts("w").await, 3);
    }

    #[tokio::test]
    async fn started_workloads_keep_their_kind_and_report_starting() {
        let table = WorkloadTable::default();
        table.started("w", Kind::Vm, "handle".to_string()).await;

        assert_eq!(table.handle("w").await.as_deref(), Some("handle"));
        assert_eq!(
            table.running().await,
            vec![("w".to_string(), Kind::Vm, "handle".to_string())]
        );
        assert!(table.pending().await.is_empty());

        table.clear_handle("w").await;
        assert_eq!(
            table.pending().await,
            vec![("w".to_string(), Phase::Starting)]
        );
    }

    #[tokio::test]
    async fn stale_entries_exclude_desired_workloads() {
        let table = WorkloadTable::default();
        table.set_phase("keep", Phase::Creating).await;
        table.set_phase("drop", Phase::Failed).await;

        let desired: HashSet<&str> = HashSet::from(["keep"]);
        let stale = table.stale(&desired).await;

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].0, "drop");
        table.remove("drop").await;
        assert!(table.stale(&desired).await.is_empty());
    }

    #[tokio::test]
    async fn adoption_records_handles_and_kinds() {
        let table = WorkloadTable::default();
        table.mark_kinds([("a".to_string(), Kind::Vm)]).await;
        table
            .adopt([("b".to_string(), Kind::Microvm, "hb".to_string())])
            .await;

        assert!(table.handle("a").await.is_none());
        assert_eq!(table.handle("b").await.as_deref(), Some("hb"));
        table.set_phase("a", Phase::Scheduled).await;
        table.clear_phase("a").await;
        assert!(table.pending().await.is_empty());
    }
}
