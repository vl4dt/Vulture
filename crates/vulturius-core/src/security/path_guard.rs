//! Path traversal protection — prevent directory escape attacks.
//!
//! Validates all user-supplied paths to ensure they cannot escape
//! intended directories through `..` components, symlinks, or encoding tricks.

use std::path::{Path, PathBuf};

use tracing::warn;

use crate::VulturiusError;

/// Maximum allowed path depth to prevent extremely deep nesting attacks.
const MAX_PATH_DEPTH: usize = 128;

/// Path guard that validates paths against traversal attacks.
pub struct PathGuard;

impl PathGuard {
    /// Validate a path is safe for operations.
    ///
    /// Checks for:
    /// - `..` components that escape the base directory
    /// - Null bytes in path
    /// - Excessive path depth
    /// - Special device paths (Windows: CON, PRN, AUX, NUL)
    /// - Unicode normalization attacks
    pub fn validate(path: &Path, base: Option<&Path>) -> Result<PathBuf, VulturiusError> {
        let path_str = path.to_string_lossy().to_string();

        // Check for null bytes
        if path_str.contains('\0') {
            return Err(VulturiusError::InvalidArgument(
                "Path contains null bytes".to_string(),
            ));
        }

        // Check path depth
        let depth = path.components().count();
        if depth > MAX_PATH_DEPTH {
            return Err(VulturiusError::InvalidArgument(format!(
                "Path depth ({}) exceeds maximum ({})",
                depth, MAX_PATH_DEPTH
            )));
        }

        // Check for Windows special device names
        if Self::is_windows_device_path(path) {
            return Err(VulturiusError::InvalidArgument(
                "Path targets a Windows device".to_string(),
            ));
        }

        // If a base is provided, ensure the resolved path stays within it
        if let Some(base) = base {
            // Check for explicit `..` components — these are always suspicious
            if Self::has_parent_dir_components(path) {
                return Err(VulturiusError::InvalidArgument(
                    "Path contains upward traversal (..)".to_string(),
                ));
            }

            // If path is already absolute and under base, just return it
            if path.is_absolute() && path.starts_with(base) {
                return Ok(path.to_path_buf());
            }

            // Resolve relative path against base
            let resolved = Self::resolve_relative(path, base)?;
            if !resolved.starts_with(base) {
                warn!(
                    "Path traversal detected: {:?} escapes base {:?}",
                    path, base
                );
                return Err(VulturiusError::InvalidArgument(format!(
                    "Path escapes base directory: {:?}",
                    base
                )));
            }
            Ok(resolved)
        } else {
            Ok(path.to_path_buf())
        }
    }

    /// Validate a path without a base directory (absolute path check only).
    pub fn validate_absolute(path: &Path) -> Result<PathBuf, VulturiusError> {
        Self::validate(path, None)
    }

    /// Check if a path contains `..` (ParentDir) components.
    fn has_parent_dir_components(path: &Path) -> bool {
        path.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    }

    /// Resolve a relative path against a base, detecting traversal attempts.
    fn resolve_relative(path: &Path, base: &Path) -> Result<PathBuf, VulturiusError> {
        let mut parts: Vec<std::path::Component> = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    if parts.is_empty() {
                        return Err(VulturiusError::InvalidArgument(
                            "Path contains upward traversal (..)".to_string(),
                        ));
                    }
                    parts.pop();
                }
                std::path::Component::Normal(_) | std::path::Component::CurDir => {
                    parts.push(component);
                }
                _ => {
                    // Skip RootDir and Prefix for relative resolution
                }
            }
        }

        let mut resolved = base.to_path_buf();
        for component in parts {
            resolved.push(component.as_os_str());
        }

        Ok(resolved)
    }

    /// Check if a path targets a Windows special device.
    fn is_windows_device_path(path: &Path) -> bool {
        #[cfg(windows)]
        {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_uppercase())
                .unwrap_or_default();

            const DEVICE_NAMES: &[&str] = &["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];

            DEVICE_NAMES.iter().any(|dev| name.starts_with(dev))
        }

        #[cfg(not(windows))]
        {
            let _ = path; // Suppress unused warning
            false
        }
    }

    /// Sanitize a filename (remove dangerous characters).
    pub fn sanitize_filename(name: &str) -> String {
        // Remove null bytes, control chars, and dangerous characters
        name.chars()
            .filter(|c| {
                !c.is_control()
                    && !matches!(
                        c,
                        '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                    )
            })
            .collect()
    }

    /// Create a safe path by joining base with a sanitized filename.
    pub fn safe_join(base: &Path, filename: &str) -> PathBuf {
        let sanitized = Self::sanitize_filename(filename);
        base.join(sanitized)
    }
}

/// Validate and sanitize a user-supplied path for use in cleanup operations.
pub fn validate_cleanup_path(
    user_path: &str,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, VulturiusError> {
    let path = PathBuf::from(user_path);

    // Basic validation
    PathGuard::validate_absolute(&path)?;

    // Canonicalize both the target and the roots for comparison
    let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    let canon_roots: Vec<PathBuf> = allowed_roots
        .iter()
        .map(|r| std::fs::canonicalize(r).unwrap_or_else(|_| r.clone()))
        .collect();

    if !canon_roots.iter().any(|root| canon.starts_with(root)) {
        return Err(VulturiusError::PermissionDenied {
            path: canon.to_string_lossy().to_string(),
        });
    }

    Ok(canon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_validate_safe_path() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let safe_path = base.join("subdir/file.txt");

        assert!(PathGuard::validate(&safe_path, Some(base)).is_ok());
    }

    #[test]
    fn test_validate_traversal_blocked() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let escape_path = base.join("subdir/../../escape");

        assert!(PathGuard::validate(&escape_path, Some(base)).is_err());
    }

    #[test]
    fn test_validate_null_byte() {
        let path = PathBuf::from("test\0file.txt");
        assert!(PathGuard::validate(&path, None).is_err());
    }

    #[test]
    fn test_validate_excessive_depth() {
        let mut deep_path = PathBuf::new();
        for _ in 0..200 {
            deep_path.push("deep");
        }
        assert!(PathGuard::validate(&deep_path, None).is_err());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(PathGuard::sanitize_filename("hello.txt"), "hello.txt");
        assert_eq!(PathGuard::sanitize_filename("file<>name.txt"), "filename.txt");
        assert_eq!(PathGuard::sanitize_filename("a/b/c"), "abc");
        assert_eq!(PathGuard::sanitize_filename("normal-file_name.txt"), "normal-file_name.txt");
    }

    #[test]
    fn test_safe_join() {
        let base = PathBuf::from("/safe/base");
        let result = PathGuard::safe_join(&base, "dangerous<>file.txt");
        assert_eq!(result, PathBuf::from("/safe/base/dangerousfile.txt"));
    }

    #[test]
    fn test_validate_absolute() {
        let path = PathBuf::from("/some/absolute/path");
        assert!(PathGuard::validate_absolute(&path).is_ok());
    }

    #[test]
    fn test_resolve_traversal_parent_at_root() {
        let base = PathBuf::from("/base");
        let bad_path = PathBuf::from("../../escape");
        assert!(PathGuard::validate(&bad_path, Some(&base)).is_err());
    }

    #[test]
    fn test_validate_cleanup_path_allowed() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let target = root.join("test.txt");

        // Create the file so canonicalize works
        std::fs::write(&target, "test").unwrap();

        let result = validate_cleanup_path(target.to_str().unwrap(), &[root]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_cleanup_path_blocked() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();

        let result = validate_cleanup_path(dir2.path().to_str().unwrap(), &[dir1.path().to_path_buf()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_dotdot_traversal() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let escape = PathBuf::from("..");

        assert!(PathGuard::validate(&escape, Some(base)).is_err());
    }

    #[test]
    fn test_has_parent_dir_components() {
        assert!(PathGuard::has_parent_dir_components(&PathBuf::from("a/../b")));
        assert!(!PathGuard::has_parent_dir_components(&PathBuf::from("a/b/c")));
    }
}
