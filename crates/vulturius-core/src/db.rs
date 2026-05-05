use sqlx::{SqlitePool, Row};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::types::*;
use crate::VulturiusError;

/// SQLite database for persisting scan results, analysis, and cleanup history.
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create or open the database at the given path.
    pub async fn new<P: AsRef<str>>(db_path: P) -> Result<Self, VulturiusError> {
        let connection_url = format!("sqlite:{}?mode=rwc", db_path.as_ref());
        let pool = SqlitePool::connect(&connection_url)
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Initialize all tables.
    pub async fn init(&self) -> Result<(), VulturiusError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS scans (
                id          TEXT PRIMARY KEY,
                started_at  TEXT NOT NULL,
                completed_at TEXT,
                scan_type   TEXT NOT NULL,
                total_files INTEGER NOT NULL DEFAULT 0,
                total_size  INTEGER NOT NULL DEFAULT 0
            );
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS file_entries (
                id            TEXT PRIMARY KEY,
                scan_id       TEXT NOT NULL,
                path          TEXT NOT NULL,
                size          INTEGER NOT NULL,
                file_type     TEXT NOT NULL,
                category      TEXT NOT NULL,
                last_accessed INTEGER NOT NULL DEFAULT 0,
                last_modified INTEGER NOT NULL DEFAULT 0,
                created       INTEGER NOT NULL DEFAULT 0,
                is_system     INTEGER NOT NULL DEFAULT 0,
                is_hidden     INTEGER NOT NULL DEFAULT 0,
                mime_type     TEXT,
                file_hash     TEXT,
                FOREIGN KEY (scan_id) REFERENCES scans(id)
            );
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS file_analysis (
                file_id          TEXT PRIMARY KEY,
                path             TEXT NOT NULL,
                confidence_score REAL NOT NULL,
                recommendation   TEXT NOT NULL,
                reason           TEXT,
                age_factor       REAL NOT NULL DEFAULT 0.0,
                location_factor  REAL NOT NULL DEFAULT 0.0,
                type_factor      REAL NOT NULL DEFAULT 0.0,
                dependency_factor REAL NOT NULL DEFAULT 0.0,
                user_pattern_factor REAL NOT NULL DEFAULT 0.0,
                llm_bonus        REAL NOT NULL DEFAULT 0.0,
                llm_reviewed     INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (file_id) REFERENCES file_entries(id)
            );
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS cleanup_history (
                id             TEXT PRIMARY KEY,
                plan_id        TEXT NOT NULL,
                executed_at    TEXT NOT NULL,
                completed      INTEGER NOT NULL,
                items_processed INTEGER NOT NULL,
                items_succeeded INTEGER NOT NULL,
                items_failed   INTEGER NOT NULL,
                space_freed    INTEGER NOT NULL DEFAULT 0
            );
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS cleanup_journal (
                id          TEXT PRIMARY KEY,
                timestamp   TEXT NOT NULL,
                operation   TEXT NOT NULL,
                path        TEXT NOT NULL,
                undo_action TEXT NOT NULL,
                reversible  INTEGER NOT NULL DEFAULT 1
            );
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS app_registry (
                id            TEXT PRIMARY KEY,
                name          TEXT NOT NULL,
                version       TEXT,
                install_path  TEXT NOT NULL,
                last_launched INTEGER,
                launch_count  INTEGER NOT NULL DEFAULT 0,
                total_size    INTEGER NOT NULL DEFAULT 0,
                usage_score   REAL NOT NULL DEFAULT 0.0,
                recommendation TEXT NOT NULL DEFAULT 'active'
            );
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        // Create indexes for performance
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_file_entries_scan_id ON file_entries(scan_id);
            CREATE INDEX IF NOT EXISTS idx_file_entries_category ON file_entries(category);
            CREATE INDEX IF NOT EXISTS idx_file_analysis_score ON file_analysis(confidence_score);
            CREATE INDEX IF NOT EXISTS idx_cleanup_history_plan ON cleanup_history(plan_id);
            CREATE INDEX IF NOT EXISTS idx_app_registry_name ON app_registry(name);
            CREATE INDEX IF NOT EXISTS idx_journal_timestamp ON cleanup_journal(timestamp);
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        // Quarantine table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS quarantine (
                id              TEXT PRIMARY KEY,
                original_path   TEXT NOT NULL,
                quarantined_path TEXT NOT NULL,
                size            INTEGER NOT NULL DEFAULT 0,
                quarantined_at  TEXT NOT NULL,
                reason          TEXT
            );
        "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        // Quarantine index (after table creation)
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_quarantine_id ON quarantine(id)")
            .execute(&self.pool)
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(())
    }

    // ─── Scan Operations ────────────────────────────────────────────────────

    /// Save a scan result to the database.
    pub async fn save_scan(&self, result: &ScanResult) -> Result<(), VulturiusError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

        // Insert scan metadata
        sqlx::query(
            "INSERT OR REPLACE INTO scans (id, started_at, completed_at, scan_type, total_files, total_size)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&result.scan_id)
        .bind(&result.started_at.to_rfc3339())
        .bind(&result.completed_at.to_rfc3339())
        .bind(&result.scan_type.to_string())
        .bind(result.total_files as i64)
        .bind(result.total_size as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        // Insert file entries in batches
        let batch_size = 500;
        for chunk in result.entries.chunks(batch_size) {
            for entry in chunk {
                sqlx::query(
                    "INSERT OR REPLACE INTO file_entries
                     (id, scan_id, path, size, file_type, category, last_accessed, last_modified, created, is_system, is_hidden, mime_type, file_hash)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&entry.id)
                .bind(&result.scan_id)
                .bind(entry.path.to_string_lossy().as_ref())
                .bind(entry.size as i64)
                .bind(&entry.file_type.to_string())
                .bind(&entry.category.to_string())
                .bind(&entry.last_accessed.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0))
                .bind(&entry.last_modified.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0))
                .bind(&entry.created.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0))
                .bind(entry.is_system as i64)
                .bind(entry.is_hidden as i64)
                .bind(&entry.mime_type)
                .bind(&entry.hash)
                .execute(&mut *tx)
                .await
                .map_err(|e| VulturiusError::Database(e.to_string()))?;
            }
        }

        tx.commit()
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get the most recent scan ID.
    pub async fn get_latest_scan_id(&self) -> Result<Option<String>, VulturiusError> {
        let row = sqlx::query("SELECT id FROM scans ORDER BY started_at DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(row.map(|r| r.get::<String, _>("id")))
    }

    /// Get a scan result by ID (reconstructs from DB).
    pub async fn get_scan(&self, scan_id: &str) -> Result<Option<ScanResult>, VulturiusError> {
        // Get scan metadata
        let scan_row = sqlx::query(
            "SELECT id, started_at, completed_at, scan_type, total_files, total_size FROM scans WHERE id = ?",
        )
        .bind(scan_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        let scan_info = match scan_row {
            Some(r) => r,
            None => return Ok(None),
        };

        // Get file entries
        let entries: Vec<FileEntry> = sqlx::query(
            "SELECT id, path, size, file_type, category, last_accessed, last_modified, created, is_system, is_hidden, mime_type, file_hash
             FROM file_entries WHERE scan_id = ?",
        )
        .bind(scan_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?
        .into_iter()
        .map(|r| FileEntry {
            id: r.get("id"),
            path: PathBuf::from(r.get::<String, _>("path")),
            size: r.get::<i64, _>("size") as u64,
            file_type: parse_file_type(&r.get::<String, _>("file_type")),
            category: parse_file_category(&r.get::<String, _>("category")),
            last_accessed: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(r.get::<i64, _>("last_accessed") as u64),
            last_modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(r.get::<i64, _>("last_modified") as u64),
            created: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(r.get::<i64, _>("created") as u64),
            permissions: 0,
            is_system: r.get::<i64, _>("is_system") != 0,
            is_hidden: r.get::<i64, _>("is_hidden") != 0,
            mime_type: r.get::<Option<String>, _>("mime_type"),
            hash: r.get::<Option<String>, _>("file_hash"),
        })
        .collect();

        Ok(Some(ScanResult {
            scan_id: scan_info.get("id"),
            started_at: chrono::DateTime::parse_from_rfc3339(&scan_info.get::<String, _>("started_at"))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            completed_at: chrono::DateTime::parse_from_rfc3339(&scan_info.get::<String, _>("completed_at"))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            entries,
            total_size: scan_info.get::<i64, _>("total_size") as u64,
            total_files: scan_info.get::<i64, _>("total_files") as usize,
            total_dirs: 0,
            errors: Vec::new(),
            scan_type: parse_scan_type(&scan_info.get::<String, _>("scan_type")),
        }))
    }

    // ─── Analysis Operations ────────────────────────────────────────────────

    /// Save analysis results.
    pub async fn save_analysis(&self, analyses: &[FileAnalysis]) -> Result<(), VulturiusError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

        for analysis in analyses {
            sqlx::query(
                "INSERT OR REPLACE INTO file_analysis
                 (file_id, path, confidence_score, recommendation, reason, age_factor, location_factor, type_factor, dependency_factor, user_pattern_factor, llm_bonus, llm_reviewed)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&analysis.file_id)
            .bind(analysis.path.to_string_lossy().as_ref())
            .bind(analysis.confidence_score)
            .bind(&analysis.recommendation.to_string())
            .bind(&analysis.reason)
            .bind(analysis.score_breakdown.age_factor)
            .bind(analysis.score_breakdown.location_factor)
            .bind(analysis.score_breakdown.type_factor)
            .bind(analysis.score_breakdown.dependency_factor)
            .bind(analysis.score_breakdown.user_pattern_factor)
            .bind(analysis.score_breakdown.llm_bonus)
            .bind(analysis.llm_reviewed as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get analysis results for a scan.
    pub async fn get_analysis(&self, file_ids: &[String]) -> Result<Vec<FileAnalysis>, VulturiusError> {
        let mut analyses = Vec::new();
        for id in file_ids {
            let row = sqlx::query(
                "SELECT file_id, path, confidence_score, recommendation, reason, age_factor, location_factor, type_factor, dependency_factor, user_pattern_factor, llm_bonus, llm_reviewed
                 FROM file_analysis WHERE file_id = ?",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;

            if let Some(r) = row {
                analyses.push(FileAnalysis {
                    file_id: r.get("file_id"),
                    path: PathBuf::from(r.get::<String, _>("path")),
                    confidence_score: r.get("confidence_score"),
                    recommendation: parse_recommendation(&r.get::<String, _>("recommendation")),
                    reason: r.get("reason"),
                    score_breakdown: ScoreBreakdown {
                        age_factor: r.get("age_factor"),
                        location_factor: r.get("location_factor"),
                        type_factor: r.get("type_factor"),
                        dependency_factor: r.get("dependency_factor"),
                        user_pattern_factor: r.get("user_pattern_factor"),
                        llm_bonus: r.get("llm_bonus"),
                    },
                    llm_reviewed: r.get::<i64, _>("llm_reviewed") != 0,
                });
            }
        }

        Ok(analyses)
    }

    // ─── Cleanup History ────────────────────────────────────────────────────

    /// Save cleanup result.
    pub async fn save_cleanup(&self, result: &CleanupResult) -> Result<(), VulturiusError> {
        sqlx::query(
            "INSERT INTO cleanup_history (id, plan_id, executed_at, completed, items_processed, items_succeeded, items_failed, space_freed)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&result.plan_id)
        .bind(&result.executed_at.to_rfc3339())
        .bind(result.completed as i64)
        .bind(result.items_processed as i64)
        .bind(result.items_succeeded as i64)
        .bind(result.items_failed as i64)
        .bind(result.space_freed as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get cleanup history.
    pub async fn get_cleanup_history(&self, limit: usize) -> Result<Vec<CleanupResult>, VulturiusError> {
        let rows = sqlx::query(
            "SELECT id, plan_id, executed_at, completed, items_processed, items_succeeded, items_failed, space_freed
             FROM cleanup_history ORDER BY executed_at DESC LIMIT ?",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| CleanupResult {
                plan_id: r.get("plan_id"),
                executed_at: chrono::DateTime::parse_from_rfc3339(&r.get::<String, _>("executed_at"))
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                completed: r.get::<i64, _>("completed") != 0,
                items_processed: r.get::<i64, _>("items_processed") as usize,
                items_succeeded: r.get::<i64, _>("items_succeeded") as usize,
                items_failed: r.get::<i64, _>("items_failed") as usize,
                space_freed: r.get::<i64, _>("space_freed") as u64,
                errors: Vec::new(),
            })
            .collect())
    }

    // ─── App Registry ───────────────────────────────────────────────────────

    /// Save an app to the registry.
    pub async fn save_app(&self, app: &AppAnalysis) -> Result<(), VulturiusError> {
        sqlx::query(
            "INSERT OR REPLACE INTO app_registry
             (id, name, version, install_path, last_launched, launch_count, total_size, usage_score, recommendation)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&app.id)
        .bind(&app.name)
        .bind(&app.version)
        .bind(app.install_path.to_string_lossy().as_ref())
        .bind(app.last_launched.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64))
        .bind(app.launch_count as i64)
        .bind(app.total_size as i64)
        .bind(app.usage_score)
        .bind(&app.recommendation.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get all apps.
    pub async fn get_all_apps(&self) -> Result<Vec<AppAnalysis>, VulturiusError> {
        let rows = sqlx::query(
            "SELECT id, name, version, install_path, last_launched, launch_count, total_size, usage_score, recommendation
             FROM app_registry",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| AppAnalysis {
                id: r.get("id"),
                name: r.get("name"),
                version: r.get("version"),
                install_path: PathBuf::from(r.get::<String, _>("install_path")),
                last_launched: r.get::<Option<i64>, _>("last_launched")
                    .map(|t| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t as u64)),
                launch_count: r.get::<i64, _>("launch_count") as usize,
                total_size: r.get::<i64, _>("total_size") as u64,
                usage_score: r.get("usage_score"),
                uninstall_safe: true,
                leftover_files: Vec::new(),
                registry_entries: Vec::new(),
                recommendation: parse_app_recommendation(&r.get::<String, _>("recommendation")),
            })
            .collect())
    }

    /// Get unused apps (not launched in N days).
    pub async fn get_unused_apps(&self, threshold_days: u64) -> Result<Vec<AppAnalysis>, VulturiusError> {
        let threshold_secs = (threshold_days * 86_400) as i64;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let rows = sqlx::query(
            "SELECT id, name, version, install_path, last_launched, launch_count, total_size, usage_score, recommendation
             FROM app_registry
             WHERE last_launched IS NULL OR (last_launched + ?) < ?",
        )
        .bind(threshold_secs)
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| AppAnalysis {
                id: r.get("id"),
                name: r.get("name"),
                version: r.get("version"),
                install_path: PathBuf::from(r.get::<String, _>("install_path")),
                last_launched: r.get::<Option<i64>, _>("last_launched")
                    .map(|t| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t as u64)),
                launch_count: r.get::<i64, _>("launch_count") as usize,
                total_size: r.get::<i64, _>("total_size") as u64,
                usage_score: r.get("usage_score"),
                uninstall_safe: true,
                leftover_files: Vec::new(),
                registry_entries: Vec::new(),
                recommendation: parse_app_recommendation(&r.get::<String, _>("recommendation")),
            })
            .collect())
    }

    // ─── Quarantine Operations ────────────────────────────────────────────────

    /// Save a quarantine entry.
    pub async fn save_quarantine_entry(&self, entry: &QuarantineEntry) -> Result<(), VulturiusError> {
        sqlx::query(
            "INSERT OR REPLACE INTO quarantine (id, original_path, quarantined_path, size, quarantined_at, reason)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.id)
        .bind(entry.original_path.to_string_lossy().as_ref())
        .bind(entry.quarantined_path.to_string_lossy().as_ref())
        .bind(entry.size as i64)
        .bind(&entry.quarantined_at.to_rfc3339())
        .bind(&entry.reason)
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get all quarantine entries.
    pub async fn get_quarantine_entries(&self) -> Result<Vec<QuarantineEntry>, VulturiusError> {
        let rows = sqlx::query(
            "SELECT id, original_path, quarantined_path, size, quarantined_at, reason
             FROM quarantine",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| QuarantineEntry {
                id: r.get("id"),
                original_path: PathBuf::from(r.get::<String, _>("original_path")),
                quarantined_path: PathBuf::from(r.get::<String, _>("quarantined_path")),
                size: r.get::<i64, _>("size") as u64,
                quarantined_at: chrono::DateTime::parse_from_rfc3339(&r.get::<String, _>("quarantined_at"))
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                reason: r.get("reason"),
            })
            .collect())
    }

    /// Get a single quarantine entry by ID.
    pub async fn get_quarantine_entry(&self, id: &str) -> Result<Option<QuarantineEntry>, VulturiusError> {
        let row = sqlx::query(
            "SELECT id, original_path, quarantined_path, size, quarantined_at, reason
             FROM quarantine WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(row.map(|r| QuarantineEntry {
            id: r.get("id"),
            original_path: PathBuf::from(r.get::<String, _>("original_path")),
            quarantined_path: PathBuf::from(r.get::<String, _>("quarantined_path")),
            size: r.get::<i64, _>("size") as u64,
            quarantined_at: chrono::DateTime::parse_from_rfc3339(&r.get::<String, _>("quarantined_at"))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            reason: r.get("reason"),
        }))
    }

    /// Remove a quarantine entry.
    pub async fn remove_quarantine_entry(&self, id: &str) -> Result<(), VulturiusError> {
        sqlx::query("DELETE FROM quarantine WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| VulturiusError::Database(e.to_string()))?;
        Ok(())
    }

    // ─── Journal Operations ───────────────────────────────────────────────────

    /// Log a journal entry.
    pub async fn log_journal(&self, entry: &JournalEntry) -> Result<(), VulturiusError> {
        sqlx::query(
            "INSERT INTO cleanup_journal (id, timestamp, operation, path, undo_action, reversible)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.id)
        .bind(&entry.timestamp.to_rfc3339())
        .bind(&entry.operation)
        .bind(entry.path.to_string_lossy().as_ref())
        .bind(&entry.undo_action)
        .bind(entry.reversible as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(())
    }

    /// Get recent journal entries.
    pub async fn get_journal_entries(&self, count: usize) -> Result<Vec<JournalEntry>, VulturiusError> {
        let rows = sqlx::query(
            "SELECT id, timestamp, operation, path, undo_action, reversible
             FROM cleanup_journal ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(count as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VulturiusError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| JournalEntry {
                id: r.get("id"),
                timestamp: chrono::DateTime::parse_from_rfc3339(&r.get::<String, _>("timestamp"))
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                operation: r.get("operation"),
                path: PathBuf::from(r.get::<String, _>("path")),
                undo_action: r.get("undo_action"),
                reversible: r.get::<i64, _>("reversible") != 0,
                size: 0,
            })
            .collect())
    }
}

// ─── Helper functions for parsing DB strings back to enums ────────────────────

fn parse_file_type(s: &str) -> FileType {
    match s {
        "dir" => FileType::Directory,
        "symlink" => FileType::Symlink,
        "special" => FileType::Special,
        _ => FileType::RegularFile,
    }
}

fn parse_file_category(s: &str) -> FileCategory {
    match s {
        "Temp" => FileCategory::Temp,
        "Cache" => FileCategory::Cache,
        "Log" => FileCategory::Log,
        "CrashDump" => FileCategory::CrashDump,
        "Thumbnail" => FileCategory::Thumbnail,
        "DownloadHelper" => FileCategory::DownloadHelper,
        "PackageCache" => FileCategory::PackageCache,
        "BrowserData" => FileCategory::BrowserData,
        "OldBackup" => FileCategory::OldBackup,
        "Duplicate" => FileCategory::Duplicate,
        "Orphaned" => FileCategory::Orphaned,
        "Document" => FileCategory::Document,
        "Image" => FileCategory::Image,
        "Video" => FileCategory::Video,
        "Audio" => FileCategory::Audio,
        "Executable" => FileCategory::Executable,
        "Library" => FileCategory::Library,
        "Config" => FileCategory::Config,
        _ => FileCategory::Unknown,
    }
}

fn parse_scan_type(s: &str) -> ScanType {
    match s {
        "quick" => ScanType::Quick,
        "full" => ScanType::Full,
        "custom" => ScanType::Custom,
        _ => ScanType::Full,
    }
}

fn parse_recommendation(s: &str) -> Recommendation {
    match s {
        "safe-to-delete" => Recommendation::SafeToDelete,
        "caution" => Recommendation::Caution,
        "do-not-delete" => Recommendation::DoNotDelete,
        "review-manually" => Recommendation::ReviewManually,
        _ => Recommendation::ReviewManually,
    }
}

fn parse_app_recommendation(s: &str) -> crate::types::AppRecommendation {
    match s {
        "unused" => crate::types::AppRecommendation::Unused,
        "rarely-used" => crate::types::AppRecommendation::RarelyUsed,
        "active" => crate::types::AppRecommendation::Active,
        "essential" => crate::types::AppRecommendation::Essential,
        _ => crate::types::AppRecommendation::Active,
    }
}
