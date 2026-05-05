use std::path::PathBuf;
use std::time::SystemTime;

use crate::platform::PlatformOps;
use crate::types::{AppAnalysis, DriveInfo};
use crate::Result;

/// Generic fallback platform implementation for unsupported OSes.
pub struct GenericPlatform;

impl GenericPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformOps for GenericPlatform {
    fn list_drives(&self) -> Result<Vec<DriveInfo>> {
        Ok(Vec::new())
    }

    fn list_installed_apps(&self) -> Result<Vec<AppAnalysis>> {
        Ok(Vec::new())
    }

    fn get_app_last_launched(&self, _app_name: &str) -> Result<Option<SystemTime>> {
        Ok(None)
    }

    fn temp_directories(&self) -> Vec<PathBuf> {
        vec![std::env::temp_dir()]
    }

    fn cache_directories(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }

    fn expand_env_vars(&self, path: &str) -> String {
        std::env::var(path).unwrap_or_else(|_| path.to_string())
    }

    fn is_system_path(&self, path: &PathBuf) -> bool {
        let path_str = path.to_string_lossy();
        path_str.starts_with("/bin/")
            || path_str.starts_with("/sbin/")
            || path_str.starts_with("/lib/")
            || path_str.starts_with("/usr/lib/")
            || path_str.starts_with("/boot/")
            || path_str.starts_with("/proc/")
            || path_str.starts_with("/sys/")
    }

    fn user_profile_dir(&self) -> Option<PathBuf> {
        self.home_dir()
    }

    fn uninstall_app(&self, _app_id: &str) -> Result<()> {
        Err(crate::VulturiusError::PlatformNotSupported(
            "App uninstall not supported on this platform".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_platform_is_system_path() {
        let platform = GenericPlatform::new();
        assert!(platform.is_system_path(&PathBuf::from("/bin/bash")));
        assert!(platform.is_system_path(&PathBuf::from("/usr/lib/libc.so")));
        assert!(platform.is_system_path(&PathBuf::from("/proc/self")));
        assert!(!platform.is_system_path(&PathBuf::from("/home/user/docs")));
        assert!(!platform.is_system_path(&PathBuf::from("/tmp/test")));
    }

    #[test]
    fn test_generic_platform_expand_env_vars() {
        let platform = GenericPlatform::new();
        // Note: set_var is unsafe in tests, use std::env::remove_var cleanup in real tests
        assert_eq!(platform.expand_env_vars("NONEXISTENT"), "NONEXISTENT");
    }
}
