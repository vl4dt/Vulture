use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::platform::{PlatformOps, create_platform_ops};
use crate::types::{ScanTarget, ScanType, *};
use crate::VulturiusError;

pub mod classifier;
pub use classifier::FileClassifier;

// ─── Scan Progress ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanProgress {
    Started { total_targets: usize },
    Progress {
        files_scanned: usize,
        dirs_scanned: usize,
        bytes_scanned: u64,
        current_path: String,
    },
    Error { path: String, message: String },
    Complete {
        total_files: usize,
        total_dirs: usize,
        total_size: u64,
        duration: Duration,
    },
    Cancelled,
}

// ─── Shared Scan State (for parallel workers) ─────────────────────────────────

struct SharedState {
    entries: Vec<FileEntry>,
    errors: Vec<ScanError>,
    total_size: u64,
    file_count: usize,
    dir_count: usize,
}

// ─── Scanner Engine ───────────────────────────────────────────────────────────

pub struct Scanner {
    config: AppConfig,
    #[allow(dead_code)]
    platform: Box<dyn PlatformOps>,
    #[allow(dead_code)]
    classifier: FileClassifier,
}

impl Scanner {
    pub fn new(config: AppConfig) -> Self {
        Self {
            classifier: FileClassifier::new(),
            config,
            platform: create_platform_ops(),
        }
    }

    /// Run a parallel scan across all targets using a tokio worker pool.
    pub async fn scan(
        &self,
        targets: Vec<ScanTarget>,
        progress_tx: Option<mpsc::Sender<ScanProgress>>,
    ) -> Result<ScanResult, VulturiusError> {
        let scan_id = Uuid::new_v4().to_string();
        let started_at = Utc::now();
        let start_time = std::time::Instant::now();

        let num_targets = targets.len();
        info!(
            "Starting parallel scan {} with {} targets",
            scan_id, num_targets
        );

        // Shared state protected by mutex for parallel workers
        let state = Arc::new(Mutex::new(SharedState {
            entries: Vec::new(),
            errors: Vec::new(),
            total_size: 0,
            file_count: 0,
            dir_count: 0,
        }));

        if let Some(tx) = &progress_tx {
            let _ = tx
                .clone()
                .send(ScanProgress::Started {
                    total_targets: num_targets,
                })
                .await;
        }

        // Spawn parallel workers — one per target
        let mut handles = Vec::new();
        for target in targets {
            let state = Arc::clone(&state);
            let progress_tx = progress_tx.clone();
            let config = self.config.clone();
            let classifier = FileClassifier::new();
            let platform = create_platform_ops();

            let handle = tokio::spawn(async move {
                Self::scan_target_parallel(
                    &target,
                    progress_tx,
                    config,
                    classifier,
                    platform,
                    state,
                )
                .await;
            });

            handles.push(handle);
        }

        // Await all workers
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Scan worker panicked: {}", e);
            }
        }

        let duration = start_time.elapsed();
        let completed_at = Utc::now();

        let final_state = state.lock().await;
        info!(
            "Parallel scan {} complete: {} files, {} bytes in {:?}",
            scan_id, final_state.file_count, final_state.total_size, duration
        );

        if let Some(tx) = &progress_tx {
            let _ = tx
                .clone()
                .send(ScanProgress::Complete {
                    total_files: final_state.file_count,
                    total_dirs: final_state.dir_count,
                    total_size: final_state.total_size,
                    duration,
                })
                .await;
        }

        Ok(ScanResult {
            scan_id,
            started_at,
            completed_at,
            entries: final_state.entries.clone(),
            total_size: final_state.total_size,
            total_files: final_state.file_count,
            total_dirs: final_state.dir_count,
            errors: final_state.errors.clone(),
            scan_type: ScanType::Full,
        })
    }

    /// Parallel worker: scan one target and merge into shared state.
    async fn scan_target_parallel(
        target: &ScanTarget,
        progress_tx: Option<mpsc::Sender<ScanProgress>>,
        config: AppConfig,
        classifier: FileClassifier,
        platform: Box<dyn PlatformOps>,
        state: Arc<Mutex<SharedState>>,
    ) {
        let walker = ignore::WalkBuilder::new(&target.path)
            .follow_links(false)
            .hidden(!target.file_types.is_some())
            .max_depth(target.max_depth)
            .build();

        for entry_result in walker {
            match entry_result {
                Ok(entry) => {
                    let path = entry.path().to_path_buf();

                    // Exclusions
                    if Self::is_excluded(&path, &target.exclusions, &platform) {
                        continue;
                    }

                    // System path protection
                    if platform.is_system_path(&path) && !config.scan.scan_system {
                        continue;
                    }

                    // Hidden files
                    if Self::is_hidden(&path) && !config.scan.scan_hidden {
                        continue;
                    }

                    let metadata = match entry.metadata() {
                        Ok(m) => m,
                        Err(e) => {
                            let err_msg = e.to_string();
                            warn!("Metadata error: {}", err_msg);
                            {
                                let mut s = state.lock().await;
                                s.errors.push(ScanError {
                                    path: path.to_string_lossy().to_string(),
                                    message: err_msg,
                                    error_type: ScanErrorType::IOError,
                                });
                            }
                            continue;
                        }
                    };

                    let is_dir = metadata.is_dir();

                    if is_dir {
                        let mut s = state.lock().await;
                        s.dir_count += 1;
                    } else {
                        let size = metadata.len();
                        let file_entry = Self::build_file_entry(
                            &path,
                            &metadata,
                            size,
                            target,
                            &classifier,
                            &platform,
                        );

                        let mut s = state.lock().await;
                        // Filter by min file size
                        if let Some(min_size) = target.min_file_size {
                            if size < min_size {
                                drop(s);
                                continue;
                            }
                        }
                        s.entries.push(file_entry);
                        s.total_size += size;
                        s.file_count += 1;

                        // Progress update every 1000 files
                        if s.file_count % 1000 == 0 {
                            if let Some(tx) = &progress_tx {
                                let _ = tx
                                    .clone()
                                    .send(ScanProgress::Progress {
                                        files_scanned: s.file_count,
                                        dirs_scanned: s.dir_count,
                                        bytes_scanned: s.total_size,
                                        current_path: path.to_string_lossy().to_string(),
                                    });
                            }
                        }
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    warn!("Scan error: {}", err_msg);
                    let mut s = state.lock().await;
                    s.errors.push(ScanError {
                        path: "unknown".to_string(),
                        message: err_msg.clone(),
                        error_type: ScanErrorType::Other,
                    });
                    if let Some(tx) = &progress_tx {
                        let _ = tx
                            .clone()
                            .send(ScanProgress::Error {
                                path: "unknown".to_string(),
                                message: err_msg,
                            });
                    }
                }
            }
        }
    }

    fn build_file_entry(
        path: &PathBuf,
        metadata: &std::fs::Metadata,
        size: u64,
        target: &ScanTarget,
        classifier: &FileClassifier,
        platform: &Box<dyn PlatformOps>,
    ) -> FileEntry {
        let file_type = if metadata.is_dir() {
            FileType::Directory
        } else if metadata.file_type().is_symlink() {
            FileType::Symlink
        } else if metadata.is_file() {
            FileType::RegularFile
        } else {
            FileType::Special
        };

        let category = classifier.classify(path);

        let mime_type: Option<String> = if file_type == FileType::RegularFile {
            mime_guess::from_path(path).first_raw().map(String::from)
        } else {
            None
        };

        let hash: Option<String> = if target.collect_hashes && file_type == FileType::RegularFile {
            Self::compute_hash_sync(path).ok()
        } else {
            None
        };

        FileEntry {
            id: Uuid::new_v4().to_string(),
            path: path.clone(),
            size,
            file_type,
            category,
            last_accessed: metadata.accessed().unwrap_or_else(|_| SystemTime::now()),
            last_modified: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
            created: metadata.created().unwrap_or_else(|_| SystemTime::now()),
            permissions: 0o644,
            is_system: platform.is_system_path(path),
            is_hidden: Self::is_hidden(path),
            mime_type,
            hash,
        }
    }

    fn is_excluded(path: &PathBuf, exclusions: &[String], platform: &Box<dyn PlatformOps>) -> bool {
        let path_str = path.to_string_lossy().to_string();
        exclusions.iter().any(|exclusion| {
            let expanded = platform.expand_env_vars(exclusion);
            path_str.contains(&expanded)
        })
    }

    fn is_hidden(path: &PathBuf) -> bool {
        path.file_name()
            .map(|name| name.to_string_lossy().starts_with('.'))
            .unwrap_or(false)
    }

    fn compute_hash_sync(path: &PathBuf) -> Result<String, VulturiusError> {
        let content = std::fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(hex::encode(hasher.finalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classifier_basic() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from("/tmp/test.tmp")),
            FileCategory::Temp
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("/var/log/syslog.log")),
            FileCategory::Log
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("Cache/data.cache")),
            FileCategory::Cache
        );
    }

    #[test]
    fn test_is_hidden() {
        assert!(Scanner::is_hidden(&PathBuf::from("/home/.hidden")));
        assert!(Scanner::is_hidden(&PathBuf::from("/tmp/.cache")));
        assert!(!Scanner::is_hidden(&PathBuf::from("/home/visible")));
        assert!(!Scanner::is_hidden(&PathBuf::from("/tmp/data.txt")));
    }
}
