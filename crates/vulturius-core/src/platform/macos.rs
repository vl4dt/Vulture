use std::path::PathBuf;
use std::time::SystemTime;

use crate::platform::PlatformOps;
use crate::types::{AppAnalysis, DriveInfo};
use crate::Result;

pub struct MacOsPlatform;

impl MacOsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformOps for MacOsPlatform {
    fn list_drives(&self) -> Result<Vec<DriveInfo>> {
        // On macOS, typically just the root volume
        let mut drives = Vec::new();
        if PathBuf::from("/").exists() {
            drives.push(DriveInfo {
                mount_point: PathBuf::from("/"),
                total_space: 0,
                available_space: 0,
                filesystem: "APFS".to_string(),
                drive_type: crate::types::DriveType::Fixed,
            });
        }
        // Check /Volumes for mounted drives
        if let Ok(entries) = std::fs::read_dir("/Volumes") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.file_name().map_or(false, |n| n != "Macintosh HD") {
                    drives.push(DriveInfo {
                        mount_point: path.clone(),
                        total_space: 0,
                        available_space: 0,
                        filesystem: "APFS".to_string(),
                        drive_type: crate::types::DriveType::Removable,
                    });
                }
            }
        }
        Ok(drives)
    }

    fn list_installed_apps(&self) -> Result<Vec<AppAnalysis>> {
        // Scan /Applications and ~/Applications for .app bundles
        let mut apps = Vec::new();
        let app_dirs = [
            PathBuf::from("/Applications"),
            PathBuf::from("~/Applications").expand_user_home(),
        ];

        for dir in &app_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "app") {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        apps.push(AppAnalysis {
                            id: uuid::Uuid::new_v4().to_string(),
                            name,
                            version: None,
                            install_path: path.clone(),
                            last_launched: None,
                            launch_count: 0,
                            total_size: dir_size(&path),
                            usage_score: 0.0,
                            uninstall_safe: true,
                            leftover_files: Vec::new(),
                            registry_entries: Vec::new(),
                            recommendation: crate::types::AppRecommendation::Active,
                        });
                    }
                }
            }
        }
        Ok(apps)
    }

    fn get_app_last_launched(&self, _app_name: &str) -> Result<Option<SystemTime>> {
        // TODO: Check LaunchServices database (LSOpenFromUrlSpec)
        Ok(None)
    }

    fn temp_directories(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("/tmp"),
            PathBuf::from("~/Library/Caches").expand_user_home(),
        ]
    }

    fn cache_directories(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("~/Library/Caches").expand_user_home(),
            PathBuf::from("/Library/Caches").expand_user_home(),
            PathBuf::from("~/Library/WebKit/Default/NetworkCache")
                .expand_user_home(),
        ]
    }

    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }

    fn expand_env_vars(&self, path: &str) -> String {
        std::env::var(path).unwrap_or_else(|_| path.to_string())
    }

    fn is_system_path(&self, path: &PathBuf) -> bool {
        let path_str = path.to_string_lossy();
        path_str.starts_with("/System/")
            || path_str.starts_with("/usr/lib/")
            || path_str.starts_with("/usr/share/")
            || path_str.starts_with("/Library/Extensions/")
    }

    fn user_profile_dir(&self) -> Option<PathBuf> {
        self.home_dir().map(|h| h.join("Library"))
    }

    fn uninstall_app(&self, _app_id: &str) -> Result<()> {
        // TODO: Remove .app bundle and associated files
        todo!("macOS app uninstall implementation")
    }
}

/// Helper: recursively calculate directory size.
fn dir_size(path: &PathBuf) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// Helper: expand ~ to home directory.
trait PathBufExt {
    fn expand_user_home(self) -> Self;
}

impl PathBufExt for PathBuf {
    fn expand_user_home(self) -> Self {
        if let Ok(home) = std::env::var("HOME") {
            if let Some(stripped) = self.to_string_lossy().strip_prefix("~/") {
                return PathBuf::from(format!("{}/{}", home, stripped));
            }
            if self.to_string_lossy() == "~" {
                return PathBuf::from(home);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_is_system_path() {
        let platform = MacOsPlatform::new();
        assert!(platform.is_system_path(&PathBuf::from("/System/Library/CoreServices")));
        assert!(platform.is_system_path(&PathBuf::from("/usr/lib/libc.dylib")));
        assert!(!platform.is_system_path(&PathBuf::from(
            "/Users/test/Documents"
        )));
    }
}
