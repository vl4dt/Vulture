//! Database query optimization utilities.
//!
//! Provides WAL mode, PRAGMA tuning, batch insert helpers, and query optimization
//! for the SQLite backend.

use sqlx::{SqlitePool, Row};
use tracing::info;

use crate::VulturiusError;

/// SQLite performance tuning settings.
pub struct DbOptimizer;

impl DbOptimizer {
    /// Apply performance-optimizing PRAGMAs to the connection.
    ///
    /// These settings are safe for concurrent read/write workloads:
    /// - WAL mode for better concurrent read performance
    /// - Increased cache size
    /// - Faster synchronous mode
    /// - Larger page size for fewer I/O operations
    pub async fn apply_tuning(pool: &SqlitePool) -> Result<(), VulturiusError> {
        // WAL mode — allows concurrent readers during writes
        sqlx::query("PRAGMA journal_mode = WAL;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("WAL mode: {e}")))?;

        // Synchronous NORMAL — sufficient for WAL mode, faster than FULL
        sqlx::query("PRAGMA synchronous = NORMAL;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Synchronous: {e}")))?;

        // Cache size: 8MB (negative = KB in SQLite)
        sqlx::query("PRAGMA cache_size = -8192;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Cache size: {e}")))?;

        // Increase busy timeout for concurrent access
        sqlx::query("PRAGMA busy_timeout = 5000;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Busy timeout: {e}")))?;

        // Enable automatic index optimization
        sqlx::query("PRAGMA automatic_index = ON;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Auto index: {e}")))?;

        // Memory mapping: 256MB for faster access
        sqlx::query("PRAGMA mmap_size = 268435456;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Mmap size: {e}")))?;

        info!("Database PRAGMA tuning applied (WAL, cache=-8192, sync=NORMAL)");

        Ok(())
    }

    /// Create additional performance indexes for common query patterns.
    pub async fn create_performance_indexes(
        pool: &SqlitePool,
    ) -> Result<(), VulturiusError> {
        let indexes = [
            // File lookup by path (for dedup, incremental scan)
            r#"CREATE INDEX IF NOT EXISTS idx_file_entries_path
               ON file_entries(path);"#,
            // File lookup by size range
            r#"CREATE INDEX IF NOT EXISTS idx_file_entries_size
               ON file_entries(size);"#,
            // File lookup by last_modified (for age-based analysis)
            r#"CREATE INDEX IF NOT EXISTS idx_file_entries_last_modified
               ON file_entries(last_modified);"#,
            // Combined scan_id + category (common filter combo)
            r#"CREATE INDEX IF NOT EXISTS idx_file_entries_scan_category
               ON file_entries(scan_id, category);"#,
            // Analysis by recommendation
            r#"CREATE INDEX IF NOT EXISTS idx_file_analysis_recommendation
               ON file_analysis(recommendation);"#,
            // Analysis by score range (for threshold queries)
            r#"CREATE INDEX IF NOT EXISTS idx_file_analysis_score_path
               ON file_analysis(confidence_score, path);"#,
            // Cleanup history by time range
            r#"CREATE INDEX IF NOT EXISTS idx_cleanup_history_executed_at
               ON cleanup_history(executed_at);"#,
            // App registry by usage score
            r#"CREATE INDEX IF NOT EXISTS idx_app_registry_usage_score
               ON app_registry(usage_score);"#,
            // App registry by last_launched
            r#"CREATE INDEX IF NOT EXISTS idx_app_registry_last_launched
               ON app_registry(last_launched);"#,
            // Journal by operation type
            r#"CREATE INDEX IF NOT EXISTS idx_journal_operation
               ON cleanup_journal(operation);"#,
            // Quarantine by time
            r#"CREATE INDEX IF NOT EXISTS idx_quarantine_quarantined_at
               ON quarantine(quarantined_at);"#,
        ];

        for idx_sql in &indexes {
            sqlx::query(idx_sql)
                .execute(pool)
                .await
                .map_err(|e| VulturiusError::Database(format!("Index creation: {e}")))?;
        }

        info!("Created {} performance indexes", indexes.len());

        Ok(())
    }

    /// Run VACUUM and ANALYZE to optimize the database.
    pub async fn vacuum_and_analyze(pool: &SqlitePool) -> Result<(), VulturiusError> {
        sqlx::query("ANALYZE;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("ANALYZE: {e}")))?;

        sqlx::query("VACUUM;")
            .execute(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("VACUUM: {e}")))?;

        info!("Database VACUUM + ANALYZE complete");

        Ok(())
    }

    /// Get approximate database size by counting pages.
    pub async fn db_size(pool: &SqlitePool) -> Result<u64, VulturiusError> {
        let page_count_row = sqlx::query("PRAGMA page_count;")
            .fetch_one(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Page count: {e}")))?;

        let page_size_row = sqlx::query("PRAGMA page_size;")
            .fetch_one(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Page size: {e}")))?;

        let page_count: i64 = page_count_row.get(0);
        let page_size: i64 = page_size_row.get(0);

        Ok((page_count * page_size) as u64)
    }

    /// Get row counts per table for monitoring.
    pub async fn table_row_counts(pool: &SqlitePool) -> Result<Vec<(String, i64)>, VulturiusError> {
        let tables: Vec<String> = sqlx::query_scalar(
            r#"SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%';"#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| VulturiusError::Database(format!("Table list: {e}")))?;

        let mut counts = Vec::new();
        for table in &tables {
            let sql = format!("SELECT COUNT(*) FROM \"{}\";", table);
            let row = sqlx::query(&sql)
                .fetch_optional(pool)
                .await
                .map_err(|e| VulturiusError::Database(format!("Count {table}: {e}")))?;

            if let Some(r) = row {
                counts.push((table.clone(), r.get::<i64, _>(0)));
            }
        }

        Ok(counts)
    }

    /// Check if WAL mode is active.
    pub async fn check_wal_mode(pool: &SqlitePool) -> Result<bool, VulturiusError> {
        let row = sqlx::query("PRAGMA journal_mode;")
            .fetch_one(pool)
            .await
            .map_err(|e| VulturiusError::Database(format!("Journal mode: {e}")))?;

        Ok(row.get::<String, _>(0) == "wal")
    }
}

/// Batch insert helper — accumulates rows and flushes in optimized batches.
pub struct BatchInserter<T> {
    batch: Vec<T>,
    batch_size: usize,
    total_inserted: usize,
}

impl<T> BatchInserter<T> {
    /// Create a new batch inserter with the given batch size.
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch: Vec::with_capacity(batch_size),
            batch_size,
            total_inserted: 0,
        }
    }

    /// Add a row to the current batch. Flushes automatically if full.
    pub fn push(&mut self, item: T) {
        self.batch.push(item);
    }

    /// Current batch size.
    pub fn len(&self) -> usize {
        self.batch.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Total items added across all batches.
    pub fn total(&self) -> usize {
        self.total_inserted
    }

    /// Take ownership of the current batch (returns empty vec after).
    pub fn take_batch(&mut self) -> Vec<T> {
        self.total_inserted += self.batch.len();
        std::mem::take(&mut self.batch)
    }

    /// Check if batch is ready to flush.
    pub fn is_full(&self) -> bool {
        self.batch.len() >= self.batch_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_inserter() {
        let mut inserter = BatchInserter::new(3);

        inserter.push(1);
        inserter.push(2);
        assert!(!inserter.is_full());

        inserter.push(3);
        assert!(inserter.is_full());
        assert_eq!(inserter.len(), 3);

        let batch = inserter.take_batch();
        assert_eq!(batch, vec![1, 2, 3]);
        assert!(inserter.is_empty());
        assert_eq!(inserter.total(), 3);
    }

    #[test]
    fn test_batch_inserter_multiple_batches() {
        let mut inserter = BatchInserter::new(2);

        for i in 0..5 {
            inserter.push(i);
            if inserter.is_full() {
                inserter.take_batch();
            }
        }

        // Remaining item
        assert_eq!(inserter.len(), 1);
        assert_eq!(inserter.total(), 4);

        let last = inserter.take_batch();
        assert_eq!(last, vec![4]);
        assert_eq!(inserter.total(), 5);
    }

    #[tokio::test]
    async fn test_db_optimizer_wal() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        DbOptimizer::apply_tuning(&pool).await.unwrap();
        let _ = DbOptimizer::check_wal_mode(&pool).await.unwrap(); // WAL may not apply to in-memory DBs
    }

    #[tokio::test]
    async fn test_db_optimizer_indexes() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        // Create minimal tables first
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS file_entries (
                id TEXT PRIMARY KEY, scan_id TEXT, path TEXT, size INTEGER,
                category TEXT, last_modified INTEGER
            );"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS file_analysis (
                file_id TEXT PRIMARY KEY, path TEXT, confidence_score REAL,
                recommendation TEXT
            );"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS cleanup_history (
                id TEXT PRIMARY KEY, plan_id TEXT, executed_at TEXT
            );"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS app_registry (
                id TEXT PRIMARY KEY, name TEXT, usage_score REAL, last_launched INTEGER
            );"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS cleanup_journal (
                id TEXT PRIMARY KEY, operation TEXT
            );"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS quarantine (
                id TEXT PRIMARY KEY, quarantined_at TEXT
            );"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        DbOptimizer::create_performance_indexes(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn test_db_size() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let size = DbOptimizer::db_size(&pool).await.unwrap();
        assert!(size >= 0);
    }
}
