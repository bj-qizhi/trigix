use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub tenant_id: String,
    pub interval_secs: u64,
    pub next_run_at: Instant,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleSummary {
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub interval_secs: u64,
    pub secs_until_next_run: u64,
}

impl ScheduleSummary {
    fn from_entry(entry: &ScheduleEntry) -> Self {
        let secs_until_next_run = entry
            .next_run_at
            .checked_duration_since(Instant::now())
            .unwrap_or(Duration::ZERO)
            .as_secs();
        Self {
            workflow_id: entry.workflow_id.clone(),
            workflow_version_id: entry.workflow_version_id.clone(),
            interval_secs: entry.interval_secs,
            secs_until_next_run,
        }
    }
}

#[derive(Clone, Default)]
pub struct ScheduleStore {
    entries: Arc<Mutex<HashMap<String, ScheduleEntry>>>,
}

impl ScheduleStore {
    pub fn register(&self, entry: ScheduleEntry) {
        let mut map = self.entries.lock().unwrap();
        map.insert(entry.workflow_version_id.clone(), entry);
    }

    pub fn unregister(&self, workflow_version_id: &str) -> bool {
        self.entries.lock().unwrap().remove(workflow_version_id).is_some()
    }

    pub fn list(&self, tenant_id: &str) -> Vec<ScheduleSummary> {
        let map = self.entries.lock().unwrap();
        let mut out: Vec<ScheduleSummary> = map
            .values()
            .filter(|e| e.tenant_id == tenant_id)
            .map(ScheduleSummary::from_entry)
            .collect();
        out.sort_by(|a, b| a.workflow_id.cmp(&b.workflow_id));
        out
    }

    /// Returns entries whose `next_run_at` has elapsed and advances them by `interval_secs`.
    pub fn take_due(&self) -> Vec<ScheduleEntry> {
        let now = Instant::now();
        let mut map = self.entries.lock().unwrap();
        let mut due = Vec::new();
        for entry in map.values_mut() {
            if entry.next_run_at <= now {
                due.push(entry.clone());
                entry.next_run_at = now + Duration::from_secs(entry.interval_secs);
            }
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_list() {
        let store = ScheduleStore::default();
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 3600,
            next_run_at: Instant::now() + Duration::from_secs(3600),
        });

        let list = store.list("t1");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].workflow_id, "wf-1");
        assert_eq!(list[0].interval_secs, 3600);

        // Other tenant sees nothing
        assert!(store.list("t2").is_empty());
    }

    #[test]
    fn unregister_removes_entry() {
        let store = ScheduleStore::default();
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 60,
            next_run_at: Instant::now() + Duration::from_secs(60),
        });
        store.unregister("v-1");
        assert!(store.list("t1").is_empty());
    }

    #[test]
    fn take_due_returns_and_bumps_overdue_entry() {
        let store = ScheduleStore::default();
        // next_run_at in the past → immediately due
        store.register(ScheduleEntry {
            workflow_id: "wf-1".into(),
            workflow_version_id: "v-1".into(),
            tenant_id: "t1".into(),
            interval_secs: 3600,
            next_run_at: Instant::now() - Duration::from_secs(1),
        });

        let due = store.take_due();
        assert_eq!(due.len(), 1);

        // After take_due, the entry is no longer due (next_run bumped by 3600s).
        let due_again = store.take_due();
        assert!(due_again.is_empty());
    }
}
