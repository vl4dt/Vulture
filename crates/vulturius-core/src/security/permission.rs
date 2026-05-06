//! Permission model — admin vs user privilege levels.
//!
//! Controls which operations require elevated privileges and enforces
//! a least-privilege model for file operations.

use std::collections::HashSet;
use std::path::Path;


/// Privilege level for the current session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PrivilegeLevel {
    /// Full administrative access — can modify system paths, uninstall apps.
    Administrator,
    /// Standard user access — limited to user directories and non-system paths.
    User,
    /// Read-only access — can scan and analyze but not modify anything.
    ReadOnly,
}

impl std::fmt::Display for PrivilegeLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrivilegeLevel::Administrator => write!(f, "administrator"),
            PrivilegeLevel::User => write!(f, "user"),
            PrivilegeLevel::ReadOnly => write!(f, "read-only"),
        }
    }
}

/// An operation that can be performed by Vulturius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Operation {
    /// Read file metadata (always allowed).
    ReadMetadata,
    /// Read file contents (for hashing, MIME detection).
    ReadContent,
    /// Scan directory recursively.
    ScanDirectory,
    /// Analyze file (compute confidence score).
    AnalyzeFile,
    /// Delete a file.
    DeleteFile,
    /// Move file to quarantine.
    QuarantineFile,
    /// Restore file from quarantine.
    RestoreFromQuarantine,
    /// Truncate a log file.
    TruncateLog,
    /// Uninstall an application.
    UninstallApp,
    /// Modify system registry.
    ModifyRegistry,
    /// Modify system paths (Windows System32, etc.).
    ModifySystemPath,
    /// Schedule automatic cleanup.
    ScheduleCleanup,
    /// Modify configuration.
    ModifyConfig,
    /// View audit log.
    ViewAuditLog,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::ReadMetadata => write!(f, "read_metadata"),
            Operation::ReadContent => write!(f, "read_content"),
            Operation::ScanDirectory => write!(f, "scan_directory"),
            Operation::AnalyzeFile => write!(f, "analyze_file"),
            Operation::DeleteFile => write!(f, "delete_file"),
            Operation::QuarantineFile => write!(f, "quarantine_file"),
            Operation::RestoreFromQuarantine => write!(f, "restore_from_quarantine"),
            Operation::TruncateLog => write!(f, "truncate_log"),
            Operation::UninstallApp => write!(f, "uninstall_app"),
            Operation::ModifyRegistry => write!(f, "modify_registry"),
            Operation::ModifySystemPath => write!(f, "modify_system_path"),
            Operation::ScheduleCleanup => write!(f, "schedule_cleanup"),
            Operation::ModifyConfig => write!(f, "modify_config"),
            Operation::ViewAuditLog => write!(f, "view_audit_log"),
        }
    }
}

/// Permission matrix — which privilege levels can perform which operations.
impl Operation {
    /// Minimum privilege level required for this operation.
    pub fn min_privilege(&self) -> PrivilegeLevel {
        match self {
            Operation::ReadMetadata => PrivilegeLevel::ReadOnly,
            Operation::ReadContent => PrivilegeLevel::ReadOnly,
            Operation::ScanDirectory => PrivilegeLevel::ReadOnly,
            Operation::AnalyzeFile => PrivilegeLevel::ReadOnly,
            Operation::ViewAuditLog => PrivilegeLevel::ReadOnly,
            Operation::DeleteFile => PrivilegeLevel::User,
            Operation::QuarantineFile => PrivilegeLevel::User,
            Operation::RestoreFromQuarantine => PrivilegeLevel::User,
            Operation::TruncateLog => PrivilegeLevel::User,
            Operation::ModifyConfig => PrivilegeLevel::User,
            Operation::ScheduleCleanup => PrivilegeLevel::User,
            Operation::UninstallApp => PrivilegeLevel::Administrator,
            Operation::ModifyRegistry => PrivilegeLevel::Administrator,
            Operation::ModifySystemPath => PrivilegeLevel::Administrator,
        }
    }

    /// Whether this operation is destructive (modifies the filesystem).
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Operation::DeleteFile
                | Operation::QuarantineFile
                | Operation::TruncateLog
                | Operation::UninstallApp
                | Operation::ModifyRegistry
                | Operation::ModifySystemPath
        )
    }

    /// Whether this operation requires user confirmation.
    pub fn requires_confirmation(&self) -> bool {
        matches!(
            self,
            Operation::DeleteFile
                | Operation::UninstallApp
                | Operation::ModifyRegistry
                | Operation::ModifySystemPath
        )
    }
}

/// Permission checker that enforces the privilege model.
#[derive(Debug, Clone)]
pub struct PermissionModel {
    /// Current privilege level.
    privilege: PrivilegeLevel,
    /// Operations that are explicitly allowed regardless of privilege.
    allowed_operations: HashSet<Operation>,
    /// Operations that are explicitly denied regardless of privilege.
    denied_operations: HashSet<Operation>,
    /// Whether to require confirmation for destructive operations.
    require_confirmation: bool,
}

impl PermissionModel {
    /// Create a new permission model at the given privilege level.
    pub fn new(privilege: PrivilegeLevel) -> Self {
        Self {
            privilege,
            allowed_operations: HashSet::new(),
            denied_operations: HashSet::new(),
            require_confirmation: true,
        }
    }

    /// Create with administrator privileges.
    pub fn administrator() -> Self {
        Self::new(PrivilegeLevel::Administrator)
    }

    /// Create with standard user privileges.
    pub fn user() -> Self {
        Self::new(PrivilegeLevel::User)
    }

    /// Create with read-only privileges.
    pub fn read_only() -> Self {
        Self::new(PrivilegeLevel::ReadOnly)
    }

    /// Allow a specific operation regardless of privilege level.
    pub fn allow_operation(mut self, op: Operation) -> Self {
        self.allowed_operations.insert(op);
        self
    }

    /// Deny a specific operation regardless of privilege level.
    pub fn deny_operation(mut self, op: Operation) -> Self {
        self.denied_operations.insert(op);
        self
    }

    /// Set whether confirmation is required for destructive operations.
    pub fn set_require_confirmation(mut self, require: bool) -> Self {
        self.require_confirmation = require;
        self
    }

    /// Check if the current privilege level allows the given operation.
    pub fn check(&self, operation: Operation) -> PermissionResult {
        // Explicit denials always win
        if self.denied_operations.contains(&operation) {
            return PermissionResult::Denied {
                reason: "Operation explicitly denied by policy".to_string(),
            };
        }

        // Explicit allowances always pass
        if self.allowed_operations.contains(&operation) {
            return PermissionResult::Allowed {
                requires_confirmation: operation.requires_confirmation() && self.require_confirmation,
            };
        }

        // Check privilege level
        let min_required = operation.min_privilege();
        if PermissionModel::privilege_dominates(self.privilege, min_required) {
            PermissionResult::Allowed {
                requires_confirmation: operation.requires_confirmation() && self.require_confirmation,
            }
        } else {
            PermissionResult::InsufficientPrivilege {
                current: self.privilege,
                required: min_required,
            }
        }
    }

    /// Check if a path operation is allowed (combines privilege + system path check).
    pub fn check_path_operation(
        &self,
        operation: Operation,
        _path: &Path,
        is_system_path: bool,
    ) -> PermissionResult {
        // If it's a system path, require admin
        if is_system_path && operation.is_destructive() {
            let result = self.check(Operation::ModifySystemPath);
            if let PermissionResult::Allowed { .. } = result {
                return self.check(operation);
            }
            return PermissionResult::Denied {
                reason: "Operation on system path requires administrator privileges".to_string(),
            };
        }

        self.check(operation)
    }

    /// Current privilege level.
    pub fn privilege_level(&self) -> PrivilegeLevel {
        self.privilege
    }

    /// Whether confirmation is required for destructive operations.
    pub fn requires_confirmation(&self, operation: Operation) -> bool {
        operation.requires_confirmation() && self.require_confirmation && operation.is_destructive()
    }

    /// Check if one privilege level dominates another.
    fn privilege_dominates(level: PrivilegeLevel, required: PrivilegeLevel) -> bool {
        match (level, required) {
            (PrivilegeLevel::Administrator, _) => true,
            (PrivilegeLevel::User, PrivilegeLevel::Administrator) => false,
            (PrivilegeLevel::User, _) => true,
            (PrivilegeLevel::ReadOnly, PrivilegeLevel::Administrator | PrivilegeLevel::User) => false,
            (PrivilegeLevel::ReadOnly, PrivilegeLevel::ReadOnly) => true,
        }
    }
}

/// Result of a permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionResult {
    /// Operation is allowed.
    Allowed {
        /// Whether the operation requires user confirmation.
        requires_confirmation: bool,
    },
    /// Operation is denied by explicit policy.
    Denied {
        /// Reason for denial.
        reason: String,
    },
    /// Current privilege level is insufficient.
    InsufficientPrivilege {
        /// Current privilege level.
        current: PrivilegeLevel,
        /// Minimum required privilege level.
        required: PrivilegeLevel,
    },
}

impl PermissionResult {
    /// Whether the operation is allowed (ignoring confirmation requirement).
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionResult::Allowed { .. })
    }

    /// Whether the operation requires confirmation.
    pub fn requires_confirmation(&self) -> bool {
        match self {
            PermissionResult::Allowed {
                requires_confirmation,
            } => *requires_confirmation,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_only_cannot_delete() {
        let pm = PermissionModel::read_only();
        assert!(!pm.check(Operation::DeleteFile).is_allowed());
    }

    #[test]
    fn test_read_only_can_scan() {
        let pm = PermissionModel::read_only();
        assert!(pm.check(Operation::ScanDirectory).is_allowed());
    }

    #[test]
    fn test_user_can_delete() {
        let pm = PermissionModel::user();
        let result = pm.check(Operation::DeleteFile);
        assert!(result.is_allowed());
        assert!(result.requires_confirmation());
    }

    #[test]
    fn test_user_cannot_modify_registry() {
        let pm = PermissionModel::user();
        assert!(!pm.check(Operation::ModifyRegistry).is_allowed());
    }

    #[test]
    fn test_admin_can_modify_registry() {
        let pm = PermissionModel::administrator();
        assert!(pm.check(Operation::ModifyRegistry).is_allowed());
    }

    #[test]
    fn test_explicit_deny_overrides() {
        let pm = PermissionModel::administrator().deny_operation(Operation::DeleteFile);
        assert!(!pm.check(Operation::DeleteFile).is_allowed());
    }

    #[test]
    fn test_explicit_allow_for_readonly() {
        let pm = PermissionModel::read_only().allow_operation(Operation::DeleteFile);
        assert!(pm.check(Operation::DeleteFile).is_allowed());
    }

    #[test]
    fn test_system_path_requires_admin() {
        let pm = PermissionModel::user();
        let result = pm.check_path_operation(
            Operation::DeleteFile,
            Path::new("C:\\Windows\\System32\\file.dll"),
            true,
        );
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_destructive_operations() {
        assert!(Operation::DeleteFile.is_destructive());
        assert!(Operation::ReadMetadata.is_destructive() == false);
        assert!(Operation::UninstallApp.is_destructive());
    }

    #[test]
    fn test_confirmation_requirements() {
        assert!(Operation::DeleteFile.requires_confirmation());
        assert!(!Operation::QuarantineFile.requires_confirmation());
        assert!(Operation::ModifyRegistry.requires_confirmation());
    }
}
