//! Audit logging — record all security-relevant operations.
//!
//! Every file operation, privilege escalation, and policy decision
//! is logged for accountability and forensic analysis.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;


/// Maximum number of audit entries kept in memory.
const MAX_AUDIT_ENTRIES: usize = 10_000;

/// Severity of an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditSeverity {
    /// Informational event (scan, analysis).
    Info,
    /// Warning event (permission denied, path blocked).
    Warning,
    /// Critical event (destructive operation, privilege escalation).
    Critical,
    /// Error event (operation failed, security violation).
    Error,
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditSeverity::Info => write!(f, "INFO"),
            AuditSeverity::Warning => write!(f, "WARNING"),
            AuditSeverity::Critical => write!(f, "CRITICAL"),
            AuditSeverity::Error => write!(f, "ERROR"),
        }
    }
}

/// Type of audit event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// File operation (read, write, delete, etc.).
    FileOperation {
        operation: String,
        path: String,
        success: bool,
    },
    /// Permission check result.
    PermissionCheck {
        operation: String,
        privilege: String,
        allowed: bool,
    },
    /// Privilege escalation attempt.
    PrivilegeEscalation {
        from: String,
        to: String,
        approved: bool,
    },
    /// Policy violation detected.
    PolicyViolation {
        reason: String,
    },
    /// Configuration change.
    ConfigChange {
        setting: String,
        old_value: String,
        new_value: String,
    },
    /// Scan started or completed.
    ScanEvent {
        scan_id: String,
        event: String,
    },
    /// Cleanup operation.
    CleanupEvent {
        plan_id: String,
        action: String,
        success: bool,
    },
    /// Quarantine action.
    QuarantineAction {
        file_id: String,
        action: String,
    },
    /// Custom event.
    Custom {
        category: String,
        message: String,
    },
}

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier for this entry.
    pub id: String,
    /// Timestamp of the event.
    pub timestamp: DateTime<Utc>,
    /// Severity level.
    pub severity: AuditSeverity,
    /// Event type with details.
    pub event_type: AuditEventType,
    /// User session identifier (if available).
    pub session_id: Option<String>,
    /// Additional context.
    pub context: Option<String>,
}

impl AuditEntry {
    /// Create a new audit entry.
    pub fn new(
        severity: AuditSeverity,
        event_type: AuditEventType,
        session_id: Option<String>,
        context: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            severity,
            event_type,
            session_id,
            context,
        }
    }
}

/// In-memory audit log with optional persistence.
#[derive(Clone)]
pub struct AuditLogger {
    /// In-memory entries (bounded).
    entries: Arc<Mutex<VecDeque<AuditEntry>>>,
    /// Whether to log to disk.
    log_to_disk: bool,
    /// Disk log path.
    log_path: Option<PathBuf>,
    /// Current session ID.
    session_id: String,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_AUDIT_ENTRIES))),
            log_to_disk: false,
            log_path: None,
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Enable disk logging at the given path.
    pub fn with_disk_logging(mut self, path: PathBuf) -> Self {
        self.log_to_disk = true;
        self.log_path = Some(path);
        self
    }

    /// Set the session ID.
    pub fn set_session_id(&mut self, session_id: String) {
        self.session_id = session_id;
    }

    /// Current session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Log an audit entry.
    pub async fn log(&self, entry: AuditEntry) {
        let mut entries = self.entries.lock().await;
        entries.push_back(entry.clone());

        // Trim if over limit
        while entries.len() > MAX_AUDIT_ENTRIES {
            entries.pop_front();
        }

        // Write to disk if enabled
        if self.log_to_disk {
            if let Some(ref path) = self.log_path {
                Self::write_to_disk(path, &entry).ok();
            }
        }
    }

    /// Convenience: log a file operation.
    pub async fn log_file_operation(
        &self,
        operation: &str,
        path: &str,
        success: bool,
    ) {
        let severity = if success {
            AuditSeverity::Info
        } else {
            AuditSeverity::Error
        };

        self.log(AuditEntry::new(
            severity,
            AuditEventType::FileOperation {
                operation: operation.to_string(),
                path: path.to_string(),
                success,
            },
            Some(self.session_id.clone()),
            None,
        ))
        .await;
    }

    /// Convenience: log a permission check.
    pub async fn log_permission_check(
        &self,
        operation: &str,
        privilege: &str,
        allowed: bool,
    ) {
        let severity = if allowed {
            AuditSeverity::Info
        } else {
            AuditSeverity::Warning
        };

        self.log(AuditEntry::new(
            severity,
            AuditEventType::PermissionCheck {
                operation: operation.to_string(),
                privilege: privilege.to_string(),
                allowed,
            },
            Some(self.session_id.clone()),
            None,
        ))
        .await;
    }

    /// Convenience: log a policy violation.
    pub async fn log_policy_violation(&self, reason: &str) {
        self.log(AuditEntry::new(
            AuditSeverity::Error,
            AuditEventType::PolicyViolation {
                reason: reason.to_string(),
            },
            Some(self.session_id.clone()),
            None,
        ))
        .await;
    }

    /// Convenience: log a scan event.
    pub async fn log_scan_event(&self, scan_id: &str, event: &str) {
        self.log(AuditEntry::new(
            AuditSeverity::Info,
            AuditEventType::ScanEvent {
                scan_id: scan_id.to_string(),
                event: event.to_string(),
            },
            Some(self.session_id.clone()),
            None,
        ))
        .await;
    }

    /// Convenience: log a cleanup event.
    pub async fn log_cleanup_event(&self, plan_id: &str, action: &str, success: bool) {
        let severity = if success {
            AuditSeverity::Info
        } else {
            AuditSeverity::Error
        };

        self.log(AuditEntry::new(
            severity,
            AuditEventType::CleanupEvent {
                plan_id: plan_id.to_string(),
                action: action.to_string(),
                success,
            },
            Some(self.session_id.clone()),
            None,
        ))
        .await;
    }

    /// Get recent entries (most recent first).
    pub async fn recent_entries(&self, count: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().await;
        entries.iter().rev().take(count).cloned().collect()
    }

    /// Get entries filtered by severity.
    pub async fn entries_by_severity(&self, severity: AuditSeverity) -> Vec<AuditEntry> {
        let entries = self.entries.lock().await;
        entries
            .iter()
            .filter(|e| e.severity == severity)
            .cloned()
            .collect()
    }

    /// Get entries filtered by time range.
    pub async fn entries_in_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<AuditEntry> {
        let entries = self.entries.lock().await;
        entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .cloned()
            .collect()
    }

    /// Total number of entries.
    pub async fn entry_count(&self) -> usize {
        self.entries.lock().await.len()
    }

    /// Clear all entries.
    pub async fn clear(&self) {
        self.entries.lock().await.clear();
    }

    /// Export entries as JSON.
    pub async fn export_json(&self) -> serde_json::Result<String> {
        let entries = self.entries.lock().await;
        serde_json::to_string_pretty(&*entries)
    }

    /// Write a single entry to disk as a JSON line.
    fn write_to_disk(path: &PathBuf, entry: &AuditEntry) -> std::io::Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let dir = path.parent();
        if let Some(parent) = dir {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let line = serde_json::to_string(entry).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;

        writeln!(file, "{}", line)?;
        Ok(())
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Security summary — quick overview of security posture.
pub struct SecuritySummary {
    pub session_id: String,
    pub total_events: usize,
    pub critical_events: usize,
    pub warnings: usize,
    pub errors: usize,
    pub policy_violations: usize,
}

impl AuditLogger {
    /// Generate a security summary.
    pub async fn security_summary(&self) -> SecuritySummary {
        let entries = self.entries.lock().await;

        let critical = entries.iter().filter(|e| e.severity == AuditSeverity::Critical).count();
        let warnings = entries.iter().filter(|e| e.severity == AuditSeverity::Warning).count();
        let errors = entries.iter().filter(|e| e.severity == AuditSeverity::Error).count();
        let violations = entries
            .iter()
            .filter(|e| matches!(e.event_type, AuditEventType::PolicyViolation { .. }))
            .count();

        SecuritySummary {
            session_id: self.session_id.clone(),
            total_events: entries.len(),
            critical_events: critical,
            warnings,
            errors,
            policy_violations: violations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_logger_basic() {
        let logger = AuditLogger::new();

        let entry = AuditEntry::new(
            AuditSeverity::Info,
            AuditEventType::Custom {
                category: "test".to_string(),
                message: "hello".to_string(),
            },
            Some("session-1".to_string()),
            None,
        );

        logger.log(entry).await;
        assert_eq!(logger.entry_count().await, 1);
    }

    #[tokio::test]
    async fn test_audit_logger_file_operation() {
        let logger = AuditLogger::new();
        logger
            .log_file_operation("delete", "/tmp/test.txt", true)
            .await;

        let entries = logger.recent_entries(10).await;
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].event_type,
            AuditEventType::FileOperation { .. }
        ));
    }

    #[tokio::test]
    async fn test_audit_logger_permission_check() {
        let logger = AuditLogger::new();
        logger
            .log_permission_check("delete_file", "user", false)
            .await;

        let entries = logger.recent_entries(10).await;
        assert!(matches!(
            entries[0].event_type,
            AuditEventType::PermissionCheck { .. }
        ));
        assert_eq!(entries[0].severity, AuditSeverity::Warning);
    }

    #[tokio::test]
    async fn test_audit_logger_policy_violation() {
        let logger = AuditLogger::new();
        logger
            .log_policy_violation("path traversal detected")
            .await;

        let summary = logger.security_summary().await;
        assert_eq!(summary.policy_violations, 1);
    }

    #[tokio::test]
    async fn test_audit_logger_max_entries() {
        let logger = AuditLogger::new();

        // Add more than MAX_AUDIT_ENTRIES
        for i in 0..MAX_AUDIT_ENTRIES + 100 {
            logger
                .log_file_operation("read", &format!("/file/{}", i), true)
                .await;
        }

        assert!(logger.entry_count().await <= MAX_AUDIT_ENTRIES);
    }

    #[tokio::test]
    async fn test_audit_logger_clear() {
        let logger = AuditLogger::new();
        logger
            .log_file_operation("read", "/file.txt", true)
            .await;
        logger.clear().await;
        assert_eq!(logger.entry_count().await, 0);
    }

    #[tokio::test]
    async fn test_audit_logger_disk_logging() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("audit.log");

        let logger = AuditLogger::new().with_disk_logging(log_path.clone());
        logger
            .log_file_operation("delete", "/tmp/test.txt", true)
            .await;

        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("delete"));
    }

    #[tokio::test]
    async fn test_audit_logger_export_json() {
        let logger = AuditLogger::new();
        logger
            .log_file_operation("read", "/file.txt", true)
            .await;

        let json = logger.export_json().await.unwrap();
        assert!(json.contains("read"));
    }

    #[tokio::test]
    async fn test_security_summary() {
        let logger = AuditLogger::new();

        logger
            .log_file_operation("read", "/file.txt", true)
            .await;
        logger
            .log_file_operation("delete", "/file.txt", false)
            .await;
        logger
            .log_policy_violation("test violation")
            .await;

        let summary = logger.security_summary().await;
        assert_eq!(summary.total_events, 3);
        assert_eq!(summary.errors, 2); // failed op + violation
        assert_eq!(summary.policy_violations, 1);
    }

    #[tokio::test]
    async fn test_entries_by_severity() {
        let logger = AuditLogger::new();

        logger
            .log(AuditEntry::new(
                AuditSeverity::Info,
                AuditEventType::Custom {
                    category: "test".to_string(),
                    message: "info".to_string(),
                },
                None,
                None,
            ))
            .await;

        logger
            .log(AuditEntry::new(
                AuditSeverity::Error,
                AuditEventType::Custom {
                    category: "test".to_string(),
                    message: "error".to_string(),
                },
                None,
                None,
            ))
            .await;

        let errors = logger.entries_by_severity(AuditSeverity::Error).await;
        assert_eq!(errors.len(), 1);
    }
}
