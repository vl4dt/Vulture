# Vulturius — API Reference

## Core Types

### `FileEntry`
Represents a scanned file with full metadata.

```rust
pub struct FileEntry {
    pub id: String,              // UUID
    pub path: PathBuf,           // Absolute path
    pub size: u64,               // File size in bytes
    pub file_type: FileType,     // RegularFile, Directory, Symlink, Special
    pub category: FileCategory,  // Temp, Cache, Log, Document, Image, etc.
    pub last_accessed: SystemTime,
    pub last_modified: SystemTime,
    pub created: SystemTime,
    pub permissions: u32,
    pub is_system: bool,
    pub is_hidden: bool,
    pub mime_type: Option<String>,
    pub hash: Option<String>,    // SHA-256 hex
}
```

### `ScanTarget`
Configuration for a scan target directory.

```rust
pub struct ScanTarget {
    pub path: PathBuf,
    pub max_depth: Option<usize>,
    pub exclusions: Vec<String>,
    pub file_types: Option<Vec<String>>,
    pub collect_hashes: bool,
}
```

### `ScanResult`
Result of a complete scan operation.

```rust
pub struct ScanResult {
    pub scan_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub entries: Vec<FileEntry>,
    pub total_size: u64,
    pub total_files: usize,
    pub total_dirs: usize,
    pub errors: Vec<ScanError>,
    pub scan_type: ScanType,
}
```

### `FileCategory`
Classification categories for files.

```rust
pub enum FileCategory {
    Temp, Cache, Log, CrashDump, Thumbnail,
    Backup, Document, Image, Video, Audio,
    Executable, Archive, Config, Unknown,
}
```

Each category has a `default_safety_score()` (0.0-1.0) indicating how safe it is to remove.

### `FileAnalysis`
Analysis result with confidence scoring.

```rust
pub struct FileAnalysis {
    pub file_id: String,
    pub confidence_score: f64,     // 0.0 - 1.0
    pub recommendation: CleanupRecommendation,
    pub factors: AnalysisFactors,
    pub llm_override: Option<String>,
}
```

### `RiskTolerance`
Scoring sensitivity levels.

```rust
pub enum RiskTolerance {
    Paranoid,    // Only flag obvious waste
    Conservative,
    Balanced,    // Default
    Aggressive,
    Nuclear,    // Flag everything
}
```

### `CleanupRecommendation`
Action recommendation from analysis.

```rust
pub enum CleanupRecommendation {
    Keep,
    Review,
    Delete,
    Quarantine,
    Truncate,
}
```

## Core Components

### `Scanner`
Parallel filesystem scanner.

```rust
pub struct Scanner {
    config: AppConfig,
    platform: Box<dyn PlatformOps>,
    classifier: FileClassifier,
}

impl Scanner {
    pub fn new(config: AppConfig) -> Self;

    pub async fn scan(
        &self,
        targets: Vec<ScanTarget>,
        progress_tx: Option<mpsc::Sender<ScanProgress>>,
    ) -> Result<ScanResult, VulturiusError>;
}
```

### `FileClassifier`
Classifies files into categories based on path patterns.

```rust
pub struct FileClassifier;

impl FileClassifier {
    pub fn new() -> Self;
    pub fn classify(&self, path: &Path) -> FileCategory;
}
```

### `Analyzer`
Multi-factor confidence scoring engine.

```rust
pub struct Analyzer {
    config: AppConfig,
}

impl Analyzer {
    pub fn new(config: AppConfig) -> Self;

    pub fn analyze(&self, entry: &FileEntry) -> FileAnalysis;

    pub fn analyze_batch(&self, entries: &[FileEntry]) -> Vec<FileAnalysis>;

    pub fn compute_confidence_score(&self, entry: &FileEntry) -> f64;
}
```

### `Cleaner`
Cleanup engine with delete, quarantine, and dry-run modes.

```rust
pub struct Cleaner {
    config: AppConfig,
    platform: Box<dyn PlatformOps>,
}

impl Cleaner {
    pub fn new(config: AppConfig) -> Self;

    pub async fn execute_plan(&self, plan: &CleanupPlan) -> Result<CleanupResult, VulturiusError>;

    pub async fn delete_file(&self, path: &Path) -> Result<(), VulturiusError>;

    pub async fn quarantine_file(&self, source: &Path) -> Result<PathBuf, VulturiusError>;

    pub async fn restore_from_quarantine(&self, quarantined_path: &Path) -> Result<PathBuf, VulturiusError>;

    pub async fn dry_run(&self, plan: &CleanupPlan) -> Result<CleanupResult, VulturiusError>;
}
```

### `UndoJournal`
Records cleanup operations for reversal.

```rust
pub struct UndoJournal;

impl UndoJournal {
    pub async fn record(&self, operation: CleanupOperation) -> Result<(), VulturiusError>;

    pub async fn undo(&self, operation_id: &str) -> Result<(), VulturiusError>;

    pub async fn list_operations(&self) -> Result<Vec<CleanupOperation>, VulturiusError>;
}
```

## LLM Integration

### `LlmProvider` Trait
```rust
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, VulturiusError>;
    async fn chat_stream(&self, request: ChatRequest) -> Result<StreamedResponse, VulturiusError>;
    fn name(&self) -> &str;
}
```

### Providers
- **OpenAI**: `OpenAiProvider::new(api_key, model, timeout)`
- **Anthropic**: `AnthropicProvider::new(api_key, model, timeout)`
- **Ollama**: `OllamaProvider::new(base_url, model)`

### `LlmKeyStore`
Encrypted API key storage.

```rust
pub struct LlmKeyStore;

impl LlmKeyStore {
    pub async fn store_key(&self, provider: &str, key: &str) -> Result<(), VulturiusError>;
    pub async fn get_key(&self, provider: &str) -> Result<String, VulturiusError>;
    pub async fn delete_key(&self, provider: &str) -> Result<(), VulturiusError>;
    pub async fn list_keys(&self) -> Result<Vec<String>, VulturiusError>;
}
```

## Performance

### `AsyncHasher`
Semaphore-gated async file hashing.

```rust
pub struct AsyncHasher;

impl AsyncHasher {
    pub fn new(config: AsyncIoConfig) -> Self;

    pub async fn hash_file(&self, path: &Path) -> Result<String, VulturiusError>;

    pub async fn hash_files(&self, paths: Vec<PathBuf>) -> Vec<Result<String, VulturiusError>>;
}
```

### `WorkerPoolTuner`
Auto-scales workers by CPU cores + available RAM.

```rust
pub struct WorkerPoolTuner;

impl WorkerPoolTuner {
    pub fn recommend_workers() -> usize;
    pub fn recommend_target_workers(max_targets: usize) -> usize;
}
```

## Security

### `PathSandbox`
Restricts file access to allowed paths.

```rust
pub struct PathSandbox;

impl PathSandbox {
    pub fn new(allowed_roots: Vec<PathBuf>) -> Self;
    pub fn check_path(&self, path: &Path) -> Result<(), VulturiusError>;
    pub fn block_path(mut self, path: PathBuf) -> Self;
}
```

### `PermissionModel`
Three-tier privilege system.

```rust
pub struct PermissionModel;

impl PermissionModel {
    pub fn administrator() -> Self;
    pub fn user() -> Self;
    pub fn read_only() -> Self;

    pub fn check(&self, operation: Operation) -> PermissionResult;
    pub fn check_path_operation(&self, operation: Operation, path: &Path, is_system: bool) -> PermissionResult;
}
```

### `PathGuard`
Validates paths against traversal attacks.

```rust
pub struct PathGuard;

impl PathGuard {
    pub fn validate(path: &Path, base: Option<&Path>) -> Result<PathBuf, VulturiusError>;
    pub fn sanitize_filename(name: &str) -> String;
    pub fn safe_join(base: &Path, filename: &str) -> PathBuf;
}
```

### `AuditLogger`
Records security-relevant events.

```rust
pub struct AuditLogger;

impl AuditLogger {
    pub fn new() -> Self;
    pub async fn log(&self, entry: AuditEntry);
    pub async fn log_file_operation(&self, operation: &str, path: &str, success: bool);
    pub async fn log_permission_check(&self, operation: &str, privilege: &str, allowed: bool);
    pub async fn security_summary(&self) -> SecuritySummary;
    pub async fn export_json(&self) -> serde_json::Result<String>;
}
```

## Scheduling

### `Scheduler`
Auto-scan and auto-cleanup coordinator.

```rust
pub struct Scheduler;

impl Scheduler {
    pub fn new(config: AppConfig) -> Self;
    pub async fn start(&self) -> Result<(), VulturiusError>;
    pub async fn stop(&self);
}
```

### `CronScheduler`
Interval/daily/weekly schedule parser.

```rust
pub struct CronScheduler;

impl CronScheduler {
    pub fn parse_interval(s: &str) -> Result<Duration, VulturiusError>;
    pub fn should_run(&self, schedule: &str) -> bool;
}
```

## Reporting

### `Reporter`
Generates HTML, JSON, and text reports.

```rust
pub struct Reporter;

impl Reporter {
    pub fn text_report(&self, scan: &ScanResult, analysis: &[FileAnalysis]) -> String;
    pub fn html_report(&self, scan: &ScanResult, analysis: &[FileAnalysis]) -> String;
    pub fn json_report(&self, scan: &ScanResult, analysis: &[FileAnalysis]) -> serde_json::Result<String>;
}
```

## Error Handling

### `VulturiusError`
Comprehensive error types.

```rust
pub enum VulturiusError {
    Io(std::io::Error),
    Scan(String),
    Analysis(String),
    Cleanup(String),
    Config(String),
    Database(String),
    Llm(String),
    PermissionDenied { path: String },
    PathBlocked { path: String },
    InvalidArgument(String),
    PlatformNotSupported(String),
    Serialization(serde_json::Error),
    ConfigParse(config::ConfigError),
    DbError(sqlx::Error),
    Timeout(String),
    Cancelled,
}

impl VulturiusError {
    pub fn is_recoverable(&self) -> bool;
    pub fn is_fatal(&self) -> bool;
}
```
