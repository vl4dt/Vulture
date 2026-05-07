use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::SystemTime;

// ─── File Entry ───────────────────────────────────────────────────────────────

/// A single file/directory discovered during scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub path: PathBuf,
    pub size: u64,
    pub file_type: FileType,
    pub category: FileCategory,
    pub last_accessed: SystemTime,
    pub last_modified: SystemTime,
    pub created: SystemTime,
    pub permissions: u32,
    pub is_system: bool,
    pub is_hidden: bool,
    pub mime_type: Option<String>,
    pub hash: Option<String>,
}

impl FileEntry {
    pub fn human_size(&self) -> String {
        humansize::format_size(self.size, humansize::DECIMAL)
    }

    pub fn age_days(&self, reference: SystemTime) -> Option<u64> {
        reference
            .duration_since(self.last_accessed)
            .ok()
            .map(|d| d.as_secs() / 86_400)
    }
}

// ─── File Type ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FileType {
    RegularFile,
    Directory,
    Symlink,
    Special,
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileType::RegularFile => write!(f, "file"),
            FileType::Directory => write!(f, "dir"),
            FileType::Symlink => write!(f, "symlink"),
            FileType::Special => write!(f, "special"),
        }
    }
}

// ─── File Category ────────────────────────────────────────────────────────────

/// Classification of a file's purpose/disposability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FileCategory {
    Temp,
    Cache,
    Log,
    CrashDump,
    Thumbnail,
    DownloadHelper,
    PackageCache,
    BrowserData,
    OldBackup,
    Duplicate,
    Orphaned,
    Document,
    Image,
    Video,
    Audio,
    Executable,
    Library,
    Config,
    Unknown,
}

impl FileCategory {
    /// Default safety score for deletion (0.0 = never delete, 1.0 = always safe).
    pub fn default_safety_score(&self) -> f64 {
        match self {
            FileCategory::Temp => 0.95,
            FileCategory::Cache => 0.90,
            FileCategory::Log => 0.80,
            FileCategory::CrashDump => 0.90,
            FileCategory::Thumbnail => 0.95,
            FileCategory::DownloadHelper => 0.90,
            FileCategory::PackageCache => 0.70,
            FileCategory::BrowserData => 0.60,
            FileCategory::OldBackup => 0.65,
            FileCategory::Duplicate => 0.80,
            FileCategory::Orphaned => 0.55,
            FileCategory::Document => 0.05,
            FileCategory::Image => 0.05,
            FileCategory::Video => 0.05,
            FileCategory::Audio => 0.05,
            FileCategory::Executable => 0.02,
            FileCategory::Library => 0.02,
            FileCategory::Config => 0.05,
            FileCategory::Unknown => 0.10,
        }
    }
}

impl fmt::Display for FileCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ─── Scan Configuration ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanTarget {
    pub path: PathBuf,
    pub scan_type: ScanType,
    pub exclusions: Vec<String>,
    pub max_depth: Option<usize>,
    pub file_types: Option<Vec<FileCategory>>,
    pub min_file_size: Option<u64>,
    pub collect_hashes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanType {
    Quick,
    Full,
    Custom,
}

impl fmt::Display for ScanType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanType::Quick => write!(f, "quick"),
            ScanType::Full => write!(f, "full"),
            ScanType::Custom => write!(f, "custom"),
        }
    }
}

// ─── Scan Result ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanError {
    pub path: String,
    pub message: String,
    pub error_type: ScanErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanErrorType {
    PermissionDenied,
    PathNotFound,
    IOError,
    Other,
}

// ─── Analysis ─────────────────────────────────────────────────────────────────

/// Risk tolerance level for cleanup recommendations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskTolerance {
    Paranoid,
    Conservative,
    Balanced,
    Aggressive,
    Nuclear,
}

impl RiskTolerance {
    /// Minimum confidence score to recommend deletion at this risk level.
    pub fn min_score(&self) -> f64 {
        match self {
            RiskTolerance::Paranoid => 0.90,
            RiskTolerance::Conservative => 0.75,
            RiskTolerance::Balanced => 0.60,
            RiskTolerance::Aggressive => 0.40,
            RiskTolerance::Nuclear => 0.10,
        }
    }
}

impl fmt::Display for RiskTolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskTolerance::Paranoid => write!(f, "paranoid"),
            RiskTolerance::Conservative => write!(f, "conservative"),
            RiskTolerance::Balanced => write!(f, "balanced"),
            RiskTolerance::Aggressive => write!(f, "aggressive"),
            RiskTolerance::Nuclear => write!(f, "nuclear"),
        }
    }
}

/// Analysis result for a single file entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub file_id: String,
    pub path: PathBuf,
    pub confidence_score: f64,
    pub recommendation: Recommendation,
    pub reason: String,
    pub score_breakdown: ScoreBreakdown,
    pub llm_reviewed: bool,
}

/// Score breakdown showing how the confidence was calculated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub age_factor: f64,
    pub location_factor: f64,
    pub type_factor: f64,
    pub dependency_factor: f64,
    pub user_pattern_factor: f64,
    pub llm_bonus: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Recommendation {
    SafeToDelete,
    Caution,
    DoNotDelete,
    ReviewManually,
}

impl fmt::Display for Recommendation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Recommendation::SafeToDelete => write!(f, "safe-to-delete"),
            Recommendation::Caution => write!(f, "caution"),
            Recommendation::DoNotDelete => write!(f, "do-not-delete"),
            Recommendation::ReviewManually => write!(f, "review-manually"),
        }
    }
}

// ─── Application Analysis ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppAnalysis {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub install_path: PathBuf,
    pub last_launched: Option<SystemTime>,
    pub launch_count: usize,
    pub total_size: u64,
    pub usage_score: f64,
    pub uninstall_safe: bool,
    pub leftover_files: Vec<PathBuf>,
    pub registry_entries: Vec<String>,
    pub recommendation: AppRecommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppRecommendation {
    Unused,
    RarelyUsed,
    Active,
    Essential,
}

impl fmt::Display for AppRecommendation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppRecommendation::Unused => write!(f, "unused"),
            AppRecommendation::RarelyUsed => write!(f, "rarely-used"),
            AppRecommendation::Active => write!(f, "active"),
            AppRecommendation::Essential => write!(f, "essential"),
        }
    }
}

// ─── Cleanup ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPlan {
    pub plan_id: String,
    pub created_at: DateTime<Utc>,
    pub risk_tolerance: RiskTolerance,
    pub items: Vec<CleanupItem>,
    pub estimated_freed_space: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupItem {
    pub file_id: String,
    pub path: PathBuf,
    pub size: u64,
    pub confidence_score: f64,
    pub operation: CleanupOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CleanupOperation {
    Delete,
    Quarantine,
    Uninstall,
    ClearCache,
    TruncateLog,
}

impl fmt::Display for CleanupOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CleanupOperation::Delete => write!(f, "delete"),
            CleanupOperation::Quarantine => write!(f, "quarantine"),
            CleanupOperation::Uninstall => write!(f, "uninstall"),
            CleanupOperation::ClearCache => write!(f, "clear-cache"),
            CleanupOperation::TruncateLog => write!(f, "truncate-log"),
        }
    }
}

/// Result of executing a cleanup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub plan_id: String,
    pub executed_at: DateTime<Utc>,
    pub completed: bool,
    pub items_processed: usize,
    pub items_succeeded: usize,
    pub items_failed: usize,
    pub space_freed: u64,
    pub errors: Vec<CleanupError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupError {
    pub path: String,
    pub message: String,
}

// ─── Quarantine ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub id: String,
    pub original_path: PathBuf,
    pub quarantined_path: PathBuf,
    pub size: u64,
    pub quarantined_at: DateTime<Utc>,
    pub reason: String,
}

// ─── Undo Journal ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub operation: String,
    pub path: PathBuf,
    pub undo_action: String,
    pub reversible: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UndoAction {
    RestoreFromFile(PathBuf),
    RestoreFromQuarantine(String),
    RestoreRegistryKey { key: String, value: String },
    RecreateDirectory,
}

// ─── System Info ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub mount_point: PathBuf,
    pub total_space: u64,
    pub available_space: u64,
    pub filesystem: String,
    pub drive_type: DriveType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DriveType {
    Fixed,
    Removable,
    Network,
    CDROM,
    Unknown,
}

impl fmt::Display for DriveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriveType::Fixed => write!(f, "fixed"),
            DriveType::Removable => write!(f, "removable"),
            DriveType::Network => write!(f, "network"),
            DriveType::CDROM => write!(f, "cdrom"),
            DriveType::Unknown => write!(f, "unknown"),
        }
    }
}
