pub mod common;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use std::path::PathBuf;
use std::time::SystemTime;

use crate::types::{AppAnalysis, DriveInfo};
use crate::Result;

/// Platform-specific operations abstracted behind a trait.
pub trait PlatformOps: Send + Sync {
    /// List all available drives/mount points.
    fn list_drives(&self) -> Result<Vec<DriveInfo>>;

    /// Detect installed applications.
    fn list_installed_apps(&self) -> Result<Vec<AppAnalysis>>;

    /// Get the last launch time for an application.
    fn get_app_last_launched(&self, app_name: &str) -> Result<Option<SystemTime>>;

    /// Get system temp directories.
    fn temp_directories(&self) -> Vec<PathBuf>;

    /// Get system cache directories.
    fn cache_directories(&self) -> Vec<PathBuf>;

    /// Get home directory.
    fn home_dir(&self) -> Option<PathBuf>;

    /// Expand environment variables in a path string.
    fn expand_env_vars(&self, path: &str) -> String;

    /// Check if a path is a system-protected path.
    fn is_system_path(&self, path: &PathBuf) -> bool;

    /// Get the current user's profile directory.
    fn user_profile_dir(&self) -> Option<PathBuf>;

    /// Uninstall an application.
    fn uninstall_app(&self, app_id: &str) -> Result<()>;
}

/// Factory function to create the platform-specific implementation.
pub fn create_platform_ops() -> Box<dyn PlatformOps> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPlatform::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOsPlatform::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxPlatform::new())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Box::new(common::GenericPlatform::new())
    }
}
