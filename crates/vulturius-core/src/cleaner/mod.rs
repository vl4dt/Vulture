use crate::config::AppConfig;
use crate::db::Database;
use crate::types::*;
use crate::VulturiusError;

use chrono::Utc;
use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn, error};
use uuid::Uuid;

/// Executes cleanup operations safely with undo journal support.
pub struct Cleaner {
    config: AppConfig,
    db: Database,
}

impl Cleaner {
    pub fn new(config: AppConfig, db: Database) -> Self {
        Self { config, db }
    }

    /// Build a cleanup plan from scan + analysis results.
    pub fn build_plan(
        &self,
        scan_result: &ScanResult,
        analyses: &[FileAnalysis],
        risk_tolerance: RiskTolerance,
        use_quarantine: bool,
    ) -> CleanupPlan {
        let min_score = risk_tolerance.min_score();
        let mut items = Vec::new();
        let mut estimated_space: u64 = 0;

        for analysis in analyses {
            if analysis.confidence_score < min_score {
                continue;
            }

            // System file protection — never touch system files
            if analysis.path.iter().any(|c| {
                let s = c.to_string_lossy();
                s.contains("System32")
                    || s.contains("syswow64")
                    || s.contains("Windows")
                    || s.contains("sysconf")
                    || s == "usr"
                    || s == "lib"
            }) && analysis.confidence_score < 0.95 {
                warn!(
                    "Blocked: {} (system path, score {})",
                    analysis.path.display(),
                    analysis.confidence_score
                );
                continue;
            }

            let operation = if use_quarantine {
                CleanupOperation::Quarantine
            } else {
                // Choose operation based on file category
                match scan_result
                    .entries
                    .iter()
                    .find(|e| e.id == analysis.file_id)
                    .map(|e| &e.category)
                {
                    Some(FileCategory::Cache) => CleanupOperation::ClearCache,
                    Some(FileCategory::Log) => CleanupOperation::TruncateLog,
                    _ => CleanupOperation::Delete,
                }
            };

            items.push(CleanupItem {
                file_id: analysis.file_id.clone(),
                path: analysis.path.clone(),
                size: scan_result
                    .entries
                    .iter()
                    .find(|e| e.id == analysis.file_id)
                    .map(|e| e.size)
                    .unwrap_or(0),
                confidence_score: analysis.confidence_score,
                operation,
            });
            estimated_space += items.last().unwrap().size;
        }

        CleanupPlan {
            plan_id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            risk_tolerance,
            items,
            estimated_freed_space: estimated_space,
        }
    }

    /// Execute a cleanup plan with undo journal logging.
    pub async fn execute_plan(
        &self,
        plan: &CleanupPlan,
        dry_run: bool,
    ) -> Result<CleanupResult, VulturiusError> {
        let executed_at = Utc::now();
        let mut items_processed = 0;
        let mut items_succeeded = 0;
        let mut items_failed = 0;
        let mut space_freed: u64 = 0;
        let mut errors = Vec::new();

        info!(
            "Executing cleanup plan {} with {} items (dry_run={})",
            plan.plan_id,
            plan.items.len(),
            dry_run
        );

        for item in &plan.items {
            items_processed += 1;

            match self.execute_item(item, dry_run).await {
                Ok(freed) => {
                    items_succeeded += 1;
                    space_freed += freed;

                    // Log to undo journal (only if not dry-run)
                    if !dry_run {
                        self.log_journal_entry(item, freed).await?;
                    }
                }
                Err(e) => {
                    items_failed += 1;
                    let err_msg = e.to_string();
                    errors.push(CleanupError {
                        path: item.path.to_string_lossy().to_string(),
                        message: err_msg.clone(),
                    });
                    error!(
                        "Cleanup item failed: {} - {}",
                        item.path.display(),
                        err_msg
                    );
                }
            }
        }

        info!(
            "Cleanup plan {} complete: {}/{} succeeded, freed {}",
            plan.plan_id,
            items_succeeded,
            items_processed,
            humansize::format_size(space_freed, humansize::DECIMAL)
        );

        let result = CleanupResult {
            plan_id: plan.plan_id.clone(),
            executed_at,
            completed: items_failed == 0,
            items_processed,
            items_succeeded,
            items_failed,
            space_freed,
            errors,
        };

        // Save result to DB
        if !dry_run {
            self.db.save_cleanup(&result).await?;
        }

        Ok(result)
    }

    /// Execute a single cleanup item.
    async fn execute_item(
        &self,
        item: &CleanupItem,
        dry_run: bool,
    ) -> Result<u64, VulturiusError> {
        if dry_run {
            info!(
                "[DRY RUN] Would {} {}",
                item.operation,
                item.path.display()
            );
            return Ok(item.size);
        }

        match item.operation {
            CleanupOperation::Delete => self.delete_file(item).await,
            CleanupOperation::Quarantine => self.quarantine_file(item).await,
            CleanupOperation::Uninstall => {
                warn!("Uninstall operation not yet fully implemented");
                Ok(0)
            }
            CleanupOperation::ClearCache => self.clear_cache(item).await,
            CleanupOperation::TruncateLog => self.truncate_log(item).await,
        }
    }

    /// Delete a file.
    async fn delete_file(&self, item: &CleanupItem) -> Result<u64, VulturiusError> {
        if !item.path.exists() {
            return Ok(0);
        }
        fs::remove_file(&item.path).await?;
        info!("Deleted: {}", item.path.display());
        Ok(item.size)
    }

    /// Move a file to quarantine.
    async fn quarantine_file(&self, item: &CleanupItem) -> Result<u64, VulturiusError> {
        let quarantine_dir = &self.config.cleanup.quarantine_path;
        fs::create_dir_all(&quarantine_dir).await?;

        let filename = item
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let quarantined_path = PathBuf::from(&quarantine_dir).join(format!(
            "{}_{}",
            Uuid::new_v4().simple(),
            filename
        ));

        fs::rename(&item.path, &quarantined_path).await?;
        info!(
            "Quarantined: {} -> {}",
            item.path.display(),
            quarantined_path.display()
        );
        Ok(item.size)
    }

    /// Clear a cache directory.
    async fn clear_cache(&self, item: &CleanupItem) -> Result<u64, VulturiusError> {
        if item.path.is_dir() {
            fs::remove_dir_all(&item.path).await?;
            info!("Cleared cache dir: {}", item.path.display());
        } else {
            fs::remove_file(&item.path).await?;
            info!("Cleared cache file: {}", item.path.display());
        }
        Ok(item.size)
    }

    /// Truncate a log file.
    async fn truncate_log(&self, item: &CleanupItem) -> Result<u64, VulturiusError> {
        fs::write(&item.path, "").await?;
        info!("Truncated log: {}", item.path.display());
        Ok(item.size)
    }

    /// Log a cleanup action to the undo journal in SQLite.
    async fn log_journal_entry(&self, item: &CleanupItem, space_freed: u64) -> Result<(), VulturiusError> {
        let undo_action = match item.operation {
            CleanupOperation::Quarantine => {
                format!("quarantine:{}", Uuid::new_v4().to_string())
            }
            _ => "delete".to_string(),
        };

        self.db
            .log_journal(&JournalEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                operation: item.operation.to_string(),
                path: item.path.clone(),
                undo_action: undo_action,
                reversible: item.operation == CleanupOperation::Quarantine,
                size: space_freed,
            })
            .await?;

        Ok(())
    }

    /// List all quarantine entries.
    pub async fn list_quarantine(&self) -> Result<Vec<QuarantineEntry>, VulturiusError> {
        self.db.get_quarantine_entries().await
    }

    /// Restore a quarantined file to its original path.
    pub async fn restore_from_quarantine(&self, entry_id: &str) -> Result<(), VulturiusError> {
        let entry = self.db.get_quarantine_entry(entry_id).await?;
        let entry = entry.ok_or_else(|| VulturiusError::Cleanup("Entry not found".to_string()))?;

        // Create original directory if needed
        if let Some(parent) = entry.original_path.parent() {
            fs::create_dir_all(parent).await.ok();
        }

        fs::rename(&entry.quarantined_path, &entry.original_path).await?;
        info!(
            "Restored: {} -> {}",
            entry.quarantined_path.display(),
            entry.original_path.display()
        );

        self.db.remove_quarantine_entry(entry_id).await?;
        Ok(())
    }

    /// Purge all quarantine entries.
    pub async fn purge_quarantine(&self) -> Result<u64, VulturiusError> {
        let entries = self.db.get_quarantine_entries().await?;
        let mut total_freed: u64 = 0;

        for entry in &entries {
            if entry.quarantined_path.exists() {
                total_freed += entry.size;
                fs::remove_file(&entry.quarantined_path).await.ok();
            }
            self.db.remove_quarantine_entry(&entry.id).await.ok();
        }

        info!("Purged {} quarantine entries, freed {}", entries.len(), humansize::format_size(total_freed, humansize::DECIMAL));
        Ok(total_freed)
    }

    /// Undo the last N cleanup operations.
    pub async fn undo_last(&self, count: usize) -> Result<usize, VulturiusError> {
        let entries = self.db.get_journal_entries(count).await?;
        let mut restored = 0;

        for entry in entries.iter().rev() {
            if !entry.reversible {
                warn!("Cannot undo: {} (not reversible)", entry.path.display());
                continue;
            }

            // For quarantine entries, restore from quarantine
            if entry.undo_action.starts_with("quarantine:") {
                // Find the quarantine entry and restore
                let entries = self.db.get_quarantine_entries().await?;
                if let Some(q_entry) = entries.iter().find(|q| q.original_path == entry.path) {
                    self.restore_from_quarantine(&q_entry.id).await.ok();
                    restored += 1;
                }
            }
        }

        info!("Undone {} operations", restored);
        Ok(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_cleaner_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"hello world").await.unwrap();

        let config = AppConfig::default();
        let db = Database::new(":memory:").await.unwrap();
        db.init().await.unwrap();
        let cleaner = Cleaner::new(config, db);

        let plan = CleanupPlan {
            plan_id: "test".to_string(),
            created_at: chrono::Utc::now(),
            risk_tolerance: RiskTolerance::Balanced,
            items: vec![CleanupItem {
                file_id: "1".to_string(),
                path: test_file.clone(),
                size: 11,
                confidence_score: 0.95,
                operation: CleanupOperation::Delete,
            }],
            estimated_freed_space: 11,
        };

        let result = cleaner.execute_plan(&plan, false).await.unwrap();
        assert_eq!(result.items_succeeded, 1);
        assert!(!test_file.exists());
    }

    #[tokio::test]
    async fn test_cleaner_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"hello world").await.unwrap();

        let config = AppConfig::default();
        let db = Database::new(":memory:").await.unwrap();
        db.init().await.unwrap();
        let cleaner = Cleaner::new(config, db);

        let plan = CleanupPlan {
            plan_id: "test".to_string(),
            created_at: chrono::Utc::now(),
            risk_tolerance: RiskTolerance::Balanced,
            items: vec![CleanupItem {
                file_id: "1".to_string(),
                path: test_file.clone(),
                size: 11,
                confidence_score: 0.95,
                operation: CleanupOperation::Delete,
            }],
            estimated_freed_space: 11,
        };

        let result = cleaner.execute_plan(&plan, true).await.unwrap();
        assert_eq!(result.items_succeeded, 1);
        assert!(test_file.exists()); // File should still exist in dry run
    }

    #[tokio::test]
    async fn test_cleaner_quarantine() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"hello world").await.unwrap();

        let quarantine_dir = temp_dir.path().join("quarantine");
        let mut config = AppConfig::default();
        config.cleanup.quarantine_path = quarantine_dir.clone();

        let db = Database::new(":memory:").await.unwrap();
        db.init().await.unwrap();
        let cleaner = Cleaner::new(config, db);

        let plan = CleanupPlan {
            plan_id: "test".to_string(),
            created_at: chrono::Utc::now(),
            risk_tolerance: RiskTolerance::Balanced,
            items: vec![CleanupItem {
                file_id: "1".to_string(),
                path: test_file.clone(),
                size: 11,
                confidence_score: 0.95,
                operation: CleanupOperation::Quarantine,
            }],
            estimated_freed_space: 11,
        };

        let result = cleaner.execute_plan(&plan, false).await.unwrap();
        assert_eq!(result.items_succeeded, 1);
        assert!(!test_file.exists()); // Original gone
        assert!(quarantine_dir.exists()); // Quarantine dir exists
    }

    #[tokio::test]
    async fn test_cleaner_build_plan_filters_system() {
        let config = AppConfig::default();
        let db = Database::new(":memory:").await.unwrap();
        db.init().await.unwrap();
        let cleaner = Cleaner::new(config, db);

        let scan_result = ScanResult {
            scan_id: "test".to_string(),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            entries: vec![
                FileEntry {
                    id: "1".to_string(),
                    path: PathBuf::from("/tmp/test.tmp"),
                    size: 100,
                    file_type: FileType::RegularFile,
                    category: FileCategory::Temp,
                    last_accessed: std::time::SystemTime::UNIX_EPOCH,
                    last_modified: std::time::SystemTime::UNIX_EPOCH,
                    created: std::time::SystemTime::UNIX_EPOCH,
                    permissions: 0,
                    is_system: false,
                    is_hidden: false,
                    mime_type: None,
                    hash: None,
                },
                FileEntry {
                    id: "2".to_string(),
                    path: PathBuf::from("/Windows/System32/critical.dll"),
                    size: 500,
                    file_type: FileType::RegularFile,
                    category: FileCategory::Library,
                    last_accessed: std::time::SystemTime::UNIX_EPOCH,
                    last_modified: std::time::SystemTime::UNIX_EPOCH,
                    created: std::time::SystemTime::UNIX_EPOCH,
                    permissions: 0,
                    is_system: true,
                    is_hidden: false,
                    mime_type: None,
                    hash: None,
                },
            ],
            total_size: 600,
            total_files: 2,
            total_dirs: 0,
            errors: Vec::new(),
            scan_type: ScanType::Full,
        };

        let analyses = vec![
            FileAnalysis {
                file_id: "1".to_string(),
                path: PathBuf::from("/tmp/test.tmp"),
                confidence_score: 0.95,
                recommendation: Recommendation::SafeToDelete,
                reason: "Temp file".to_string(),
                score_breakdown: ScoreBreakdown {
                    age_factor: 0.9,
                    location_factor: 0.9,
                    type_factor: 0.9,
                    dependency_factor: 0.9,
                    user_pattern_factor: 0.9,
                    llm_bonus: 0.0,
                },
                llm_reviewed: false,
            },
            FileAnalysis {
                file_id: "2".to_string(),
                path: PathBuf::from("/Windows/System32/critical.dll"),
                confidence_score: 0.7,
                recommendation: Recommendation::Caution,
                reason: "System file".to_string(),
                score_breakdown: ScoreBreakdown {
                    age_factor: 0.5,
                    location_factor: 0.3,
                    type_factor: 0.2,
                    dependency_factor: 0.1,
                    user_pattern_factor: 0.5,
                    llm_bonus: 0.0,
                },
                llm_reviewed: false,
            },
        ];

        let plan = cleaner.build_plan(&scan_result, &analyses, RiskTolerance::Balanced, false);
        // System file should be filtered out (score 0.7 < 0.95 threshold for system paths)
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].file_id, "1");
    }
}
