//! Incremental scan support — skip unchanged directories to speed up repeated scans.
//!
//! Uses directory mtime as a change indicator. If a directory's mtime is unchanged
//! since the last scan, its contents are skipped entirely.

use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tracing::info;

/// Snapshot of directory mtimes from a previous scan.
#[derive(Debug, Clone)]
pub struct DirectorySnapshot {
    /// Map of directory path -> mtime (as unix epoch seconds).
    pub dir_mtines: HashMap<PathBuf, u64>,
    /// When this snapshot was taken.
    pub taken_at: SystemTime,
}

impl Default for DirectorySnapshot {
    fn default() -> Self {
        Self {
            dir_mtines: HashMap::new(),
            taken_at: SystemTime::now(),
        }
    }
}

impl DirectorySnapshot {
    /// Create a new empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the mtime of a directory.
    pub fn record_dir(&mut self, dir: &Path, metadata: &Metadata) {
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.dir_mtines.insert(dir.to_path_buf(), mtime);
    }

    /// Check if a directory has changed since this snapshot.
    /// Returns `true` if the directory is unchanged (can be skipped).
    pub fn is_dir_unchanged(&self, dir: &Path, metadata: &Metadata) -> bool {
        let current_mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let stored_mtime = self.dir_mtines.get(dir).copied();

        match stored_mtime {
            Some(stored) => stored == current_mtime,
            None => false, // New directory, must scan
        }
    }

    /// Check age of snapshot — if older than threshold, force full rescan.
    pub fn is_stale(&self, max_age_seconds: u64) -> bool {
        SystemTime::now()
            .duration_since(self.taken_at)
            .map(|d| d.as_secs() > max_age_seconds)
            .unwrap_or(true)
    }

    /// Number of directories tracked.
    pub fn dir_count(&self) -> usize {
        self.dir_mtines.len()
    }
}

/// Incremental scan state — tracks which dirs can be skipped.
#[derive(Debug)]
pub struct IncrementalScanState {
    /// Previous scan snapshot (if any).
    previous_snapshot: Option<DirectorySnapshot>,
    /// Current scan snapshot being built.
    current_snapshot: DirectorySnapshot,
    /// Directories skipped in this scan.
    skipped_dirs: usize,
    /// Files skipped (estimated, from cached dir counts).
    skipped_files_estimate: usize,
    /// Maximum snapshot age before forcing full rescan (default: 24h).
    max_snapshot_age: u64,
    /// Whether incremental mode is enabled.
    enabled: bool,
}

impl IncrementalScanState {
    /// Create a new incremental scan state.
    pub fn new(enabled: bool, previous_snapshot: Option<DirectorySnapshot>) -> Self {
        Self {
            previous_snapshot,
            current_snapshot: DirectorySnapshot::new(),
            skipped_dirs: 0,
            skipped_files_estimate: 0,
            max_snapshot_age: 24 * 3600, // 24 hours
            enabled,
        }
    }

    /// Create with full rescan (no previous snapshot).
    pub fn full_scan() -> Self {
        Self::new(true, None)
    }

    /// Set maximum snapshot age.
    pub fn with_max_age(mut self, seconds: u64) -> Self {
        self.max_snapshot_age = seconds;
        self
    }

    /// Check if a directory can be skipped.
    /// Returns `true` if the directory should be skipped (unchanged).
    pub fn check_dir(&mut self, dir: &Path, metadata: &Metadata) -> bool {
        if !self.enabled {
            return false;
        }

        // Record in current snapshot regardless
        self.current_snapshot.record_dir(dir, metadata);

        // Check against previous snapshot
        if let Some(ref prev) = self.previous_snapshot {
            if prev.is_stale(self.max_snapshot_age) {
                return false; // Snapshot too old, force rescan
            }
            if prev.is_dir_unchanged(dir, metadata) {
                self.skipped_dirs += 1;
                return true;
            }
        }

        false
    }

    /// Finalize the scan and produce the new snapshot.
    pub fn finalize(self) -> DirectorySnapshot {
        info!(
            "Incremental scan: skipped {} directories",
            self.skipped_dirs
        );
        self.current_snapshot
    }

    /// Statistics about skipping.
    pub fn stats(&self) -> (usize, usize) {
        (self.skipped_dirs, self.skipped_files_estimate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_snapshot_record_and_check() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        let meta = fs::metadata(path).unwrap();
        let mut snapshot = DirectorySnapshot::new();
        snapshot.record_dir(path, &meta);

        assert!(snapshot.is_dir_unchanged(path, &meta));
        assert_eq!(snapshot.dir_count(), 1);
    }

    #[test]
    fn test_snapshot_new_dir() {
        let dir = tempdir().unwrap();
        let snapshot = DirectorySnapshot::new();

        let meta = fs::metadata(dir.path()).unwrap();
        assert!(!snapshot.is_dir_unchanged(dir.path(), &meta));
    }

    #[test]
    fn test_incremental_disabled() {
        let mut state = IncrementalScanState::new(false, None);
        let dir = tempdir().unwrap();
        let meta = fs::metadata(dir.path()).unwrap();

        assert!(!state.check_dir(dir.path(), &meta));
    }

    #[test]
    fn test_full_scan_no_skip() {
        let mut state = IncrementalScanState::full_scan();
        let dir = tempdir().unwrap();
        let meta = fs::metadata(dir.path()).unwrap();

        assert!(!state.check_dir(dir.path(), &meta));
    }

    #[test]
    fn test_snapshot_stale() {
        let mut snapshot = DirectorySnapshot::new();
        snapshot.taken_at = SystemTime::UNIX_EPOCH;

        assert!(snapshot.is_stale(3600)); // 1 hour threshold
    }
}
