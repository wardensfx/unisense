use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::sysinfo::Snapshot;

#[derive(Default)]
pub struct AppState {
    snapshots: Mutex<HashMap<String, Snapshot>>,
    next_id: AtomicU64,
}

impl AppState {
    pub fn store_snapshot(&self, snapshot: Snapshot) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let key = format!("snap-{id}");
        self.snapshots.lock().unwrap().insert(key.clone(), snapshot);
        key
    }

    pub fn with_two_snapshots<T>(
        &self,
        a: &str,
        b: &str,
        f: impl FnOnce(&Snapshot, &Snapshot) -> T,
    ) -> Option<T> {
        let snapshots = self.snapshots.lock().unwrap();
        let snap_a = snapshots.get(a)?;
        let snap_b = snapshots.get(b)?;
        Some(f(snap_a, snap_b))
    }

    pub fn clear_snapshots(&self) {
        self.snapshots.lock().unwrap().clear();
    }
}
