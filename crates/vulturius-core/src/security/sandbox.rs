//! Sandboxed file operations — restrict all file access to a set of allowed paths.
//!
//! Any path outside the sandbox is rejected before any I/O is attempted.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::warn;

use crate::VulturiusError;

/// A sandbox that restricts file operations to a set of allowed root paths.
#[derive(Debug, Clone)]
pub struct PathSandbox {
    /// Allowed root paths — any path under these roots is permitted.
    allowed_roots: Vec<PathBuf>,
    /// Explicitly blocked paths (even if under an allowed root).
    blocked_paths: HashSet<PathBuf>,
    /// Whether sandbox enforcement is active.
    enabled: bool,
}

impl PathSandbox {
    /// Create a new sandbox with the given allowed roots.
    pub fn new(allowed_roots: Vec<PathBuf>) -> Self {
        Self {
            allowed_roots,
            blocked_paths: HashSet::new(),
            enabled: true,
        }
    }

    /// Create an unrestricted sandbox (all paths allowed).
    pub fn unrestricted() -> Self {
        Self {
            allowed_roots: Vec::new(),
            blocked_paths: HashSet::new(),
            enabled: false,
        }
    }

    /// Add an allowed root path.
    pub fn allow_root(mut self, path: PathBuf) -> Self {
        self.allowed_roots.push(path);
        self
    }

    /// Add a blocked path (takes precedence over allowed roots).
    pub fn block_path(mut self, path: PathBuf) -> Self {
        self.blocked_paths.insert(path);
        self
    }

    /// Enable or disable sandbox enforcement.
    pub fn set_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Check if a path is allowed by the sandbox.
    ///
    /// Returns `Ok(())` if the path is allowed, or an error if blocked.
    pub fn check_path(&self, path: &Path) -> Result<(), VulturiusError> {
        if !self.enabled {
            return Ok(());
        }

        // Canonicalize for consistent comparison
        let canon_path = Self::canonicalize(path);

        // Check blocked paths first (highest priority)
        if self.is_blocked(&canon_path) {
            return Err(VulturiusError::PermissionDenied {
                path: canon_path.to_string_lossy().to_string(),
            });
        }

        // Check allowed roots
        if self.allowed_roots.is_empty() {
            // No roots configured = deny all
            return Err(VulturiusError::PermissionDenied {
                path: canon_path.to_string_lossy().to_string(),
            });
        }

        if self.is_under_allowed_root(&canon_path) {
            Ok(())
        } else {
            warn!(
                "Sandbox violation: path not under allowed roots: {:?}",
                canon_path
            );
            Err(VulturiusError::PermissionDenied {
                path: canon_path.to_string_lossy().to_string(),
            })
        }
    }

    /// Check multiple paths, returns first error if any.
    pub fn check_paths(&self, paths: &[PathBuf]) -> Result<(), VulturiusError> {
        for path in paths {
            self.check_path(path)?;
        }
        Ok(())
    }

    /// Whether the sandbox is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Number of allowed roots.
    pub fn allowed_root_count(&self) -> usize {
        self.allowed_roots.len()
    }

    /// Number of blocked paths.
    pub fn blocked_path_count(&self) -> usize {
        self.blocked_paths.len()
    }

    /// Check if a path is explicitly blocked.
    fn is_blocked(&self, path: &Path) -> bool {
        // Exact match
        if self.blocked_paths.contains(path) {
            return true;
        }

        // Check if path is under a blocked directory
        for blocked in &self.blocked_paths {
            if path.starts_with(blocked) {
                return true;
            }
        }

        false
    }

    /// Check if a path is under any allowed root.
    fn is_under_allowed_root(&self, path: &Path) -> bool {
        self.allowed_roots.iter().any(|root| path.starts_with(root))
    }

    /// Canonicalize a path for consistent comparison.
    fn canonicalize(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Builder for creating a sandbox with common defaults.
pub struct SandboxBuilder {
    allowed_roots: Vec<PathBuf>,
    blocked_paths: HashSet<PathBuf>,
    enabled: bool,
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self {
            allowed_roots: Vec::new(),
            blocked_paths: HashSet::new(),
            enabled: true,
        }
    }
}

impl SandboxBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allowed_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.allowed_roots = roots;
        self
    }

    pub fn blocked_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.blocked_paths = paths.into_iter().collect();
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn build(self) -> PathSandbox {
        PathSandbox {
            allowed_roots: self.allowed_roots,
            blocked_paths: self.blocked_paths,
            enabled: self.enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sandbox_allows_path_under_root() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        let sandbox = PathSandbox::new(vec![root.clone()]);
        assert!(sandbox.check_path(&root.join("subdir/file.txt")).is_ok());
    }

    #[test]
    fn test_sandbox_blocks_path_outside_root() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        let sandbox = PathSandbox::new(vec![root]);
        let outside = tempdir().unwrap();
        assert!(sandbox.check_path(outside.path()).is_err());
    }

    #[test]
    fn test_sandbox_blocklist_overrides_allowlist() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let blocked = root.join("blocked_dir");

        let sandbox = PathSandbox::new(vec![root.clone()])
            .block_path(blocked.clone());

        assert!(sandbox.check_path(&blocked).is_err());
        assert!(sandbox.check_path(&root.join("allowed.txt")).is_ok());
    }

    #[test]
    fn test_unrestricted_sandbox() {
        let sandbox = PathSandbox::unrestricted();
        let dir = tempdir().unwrap();
        assert!(sandbox.check_path(dir.path()).is_ok());
    }

    #[test]
    fn test_disabled_sandbox() {
        let sandbox = PathSandbox::new(vec![PathBuf::from("/nonexistent")])
            .set_enabled(false);
        let dir = tempdir().unwrap();
        assert!(sandbox.check_path(dir.path()).is_ok());
    }

    #[test]
    fn test_empty_roots_deny_all() {
        let sandbox = PathSandbox::new(vec![]);
        let dir = tempdir().unwrap();
        assert!(sandbox.check_path(dir.path()).is_err());
    }
}
