//! Async I/O tuning for the scanner.
//!
//! Provides async hashing, buffered reads, and configurable worker pool sizing
//! to maximize throughput on both SSD and HDD storage.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use sha2::{Sha256, Digest};
use tokio::sync::Semaphore;
use tracing::info;

use crate::VulturiusError;

/// Default buffer size for hashing reads (128 KB).
const DEFAULT_HASH_BUFFER_SIZE: usize = 128 * 1024;

/// Maximum concurrent hash operations (tuned by storage type).
const DEFAULT_MAX_CONCURRENT_HASHES: usize = 8;

/// Configuration for async I/O operations.
#[derive(Debug, Clone)]
pub struct AsyncIoConfig {
    /// Buffer size for hashing file reads.
    pub hash_buffer_size: usize,
    /// Maximum concurrent hash operations.
    pub max_concurrent_hashes: usize,
    /// Whether to use async I/O for hashing (vs sync).
    pub use_async_hashing: bool,
    /// Estimated storage type (affects concurrency tuning).
    pub storage_type: StorageType,
}

impl Default for AsyncIoConfig {
    fn default() -> Self {
        Self {
            hash_buffer_size: DEFAULT_HASH_BUFFER_SIZE,
            max_concurrent_hashes: DEFAULT_MAX_CONCURRENT_HASHES,
            use_async_hashing: true,
            storage_type: StorageType::Auto,
        }
    }
}

/// Detected storage type for tuning I/O concurrency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageType {
    Ssd,
    Hdd,
    Network,
    Auto,
}

impl StorageType {
    /// Recommended max concurrent hashes for this storage type.
    pub fn recommended_concurrency(&self) -> usize {
        match self {
            StorageType::Ssd => 16,
            StorageType::Hdd => 4,
            StorageType::Network => 2,
            StorageType::Auto => DEFAULT_MAX_CONCURRENT_HASHES,
        }
    }
}

/// Async hasher with semaphore-based concurrency control.
#[derive(Clone)]
pub struct AsyncHasher {
    config: Arc<AsyncIoConfig>,
    semaphore: Arc<Semaphore>,
}

impl AsyncHasher {
    /// Create a new async hasher with the given config.
    pub fn new(config: AsyncIoConfig) -> Self {
        let concurrency = config
            .storage_type
            .recommended_concurrency()
            .min(config.max_concurrent_hashes);

        info!(
            "AsyncHasher: concurrency={}, buffer={}KB, storage={:?}",
            concurrency,
            config.hash_buffer_size / 1024,
            config.storage_type
        );

        Self {
            config: Arc::new(config),
            semaphore: Arc::new(Semaphore::new(concurrency)),
        }
    }

    /// Hash a file asynchronously with buffered reads and concurrency control.
    pub async fn hash_file(&self, path: &Path) -> Result<String, VulturiusError> {
        // Acquire semaphore slot
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| VulturiusError::Scan(format!("Hasher semaphore error: {e}")))?;

        // Spawn blocking I/O on a dedicated thread
        let path = path.to_path_buf();
        let buffer_size = self.config.hash_buffer_size;

        let result = tokio::task::spawn_blocking(move || {
            Self::hash_file_sync(&path, buffer_size)
        })
        .await
        .map_err(|e| VulturiusError::Scan(format!("Hasher task join error: {e}")))?;

        result
    }

    /// Sync helper: hash a file with buffered reads.
    pub fn hash_file_sync(path: &Path, buffer_size: usize) -> Result<String, VulturiusError> {
        let mut file = File::open(path).map_err(|e| {
            VulturiusError::Scan(format!("Cannot open file for hashing: {e}"))
        })?;

        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; buffer_size];

        loop {
            let bytes_read = file.read(&mut buffer).map_err(|e| {
                VulturiusError::Scan(format!("Read error during hashing: {e}"))
            })?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        Ok(hex::encode(hasher.finalize()))
    }

    /// Hash multiple files concurrently.
    pub async fn hash_files(
        &self,
        paths: Vec<std::path::PathBuf>,
    ) -> Vec<Result<String, VulturiusError>> {
        let mut handles = Vec::new();

        for path in paths {
            let hasher = self.clone();
            let handle = tokio::spawn(async move { hasher.hash_file(&path).await });
            handles.push(handle);
        }

        futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap_or_else(|e| Err(VulturiusError::Scan(format!("Task error: {e}")))))
            .collect()
    }
}

/// Buffered reader for large file operations.
pub struct BufferedFileReader {
    buffer_size: usize,
}

impl BufferedFileReader {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    /// Read a file in chunks and process each chunk with a callback.
    pub fn read_chunks<F>(&self, path: &Path, mut callback: F) -> Result<usize, VulturiusError>
    where
        F: FnMut(&[u8]) -> Result<(), VulturiusError>,
    {
        let mut file = File::open(path).map_err(|e| {
            VulturiusError::Scan(format!("Cannot open file: {e}"))
        })?;

        let mut buffer = vec![0u8; self.buffer_size];
        let mut total_bytes = 0usize;

        loop {
            let bytes_read = file.read(&mut buffer).map_err(|e| {
                VulturiusError::Scan(format!("Read error: {e}"))
            })?;

            if bytes_read == 0 {
                break;
            }

            callback(&buffer[..bytes_read])?;
            total_bytes += bytes_read;
        }

        Ok(total_bytes)
    }

    /// Compute file size without reading content.
    pub fn file_size(path: &Path) -> Result<u64, VulturiusError> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| VulturiusError::Scan(format!("Metadata error: {e}")))
    }
}

/// Worker pool size tuner — picks optimal concurrency based on system info.
pub struct WorkerPoolTuner;

impl WorkerPoolTuner {
    /// Recommend worker pool size based on CPU cores and available memory.
    pub fn recommend_workers() -> usize {
        let num_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1);
        let available_memory = Self::available_memory_gb();

        // Scale workers by cores, capped by available memory
        let memory_factor = (available_memory / 2.0).clamp(1.0, 4.0) as usize;
        (num_cores * memory_factor).min(64)
    }

    /// Recommend scan target parallelism (one worker per target, but bounded).
    pub fn recommend_target_workers(max_targets: usize) -> usize {
        let max_workers = Self::recommend_workers();
        max_targets.min(max_workers)
    }

    fn available_memory_gb() -> f64 {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.available_memory() as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_file_sync() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello, world!").unwrap();
        let path = file.path().to_path_buf();

        let hash = AsyncHasher::hash_file_sync(&path, 4096).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_hash_deterministic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "deterministic test data").unwrap();
        let path = file.path().to_path_buf();

        let hash1 = AsyncHasher::hash_file_sync(&path, 1024).unwrap();
        let hash2 = AsyncHasher::hash_file_sync(&path, 4096).unwrap();
        assert_eq!(hash1, hash2); // Same content, same hash regardless of buffer
    }

    #[test]
    fn test_buffered_reader() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test data for buffered reader").unwrap();
        let path = file.path().to_path_buf();

        let reader = BufferedFileReader::new(8);
        let mut total = 0usize;

        reader
            .read_chunks(&path, |chunk| {
                total += chunk.len();
                Ok(())
            })
            .unwrap();

        assert!(total > 0);
    }

    #[test]
    fn test_storage_type_concurrency() {
        assert_eq!(StorageType::Ssd.recommended_concurrency(), 16);
        assert_eq!(StorageType::Hdd.recommended_concurrency(), 4);
        assert_eq!(StorageType::Network.recommended_concurrency(), 2);
    }

    #[test]
    fn test_worker_pool_tuner() {
        let workers = WorkerPoolTuner::recommend_workers();
        assert!(workers >= 1);
        assert!(workers <= 64);
    }

    #[tokio::test]
    async fn test_async_hasher() {
        let config = AsyncIoConfig::default();
        let hasher = AsyncHasher::new(config);

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "async test").unwrap();
        let path = file.path().to_path_buf();

        let hash = hasher.hash_file(&path).await.unwrap();
        assert_eq!(hash.len(), 64);
    }
}
