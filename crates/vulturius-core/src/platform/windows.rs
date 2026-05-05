use std::path::PathBuf;
use std::time::SystemTime;

use crate::platform::PlatformOps;
use crate::types::{AppAnalysis, DriveInfo, DriveType};
use crate::Result;

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformOps for WindowsPlatform {
    fn list_drives(&self) -> Result<Vec<DriveInfo>> {
        let mut drives = Vec::new();
        // Enumerate drives A-Z
        for letter in b'A'..=b'Z' {
            let drive_root = format!("{}:\\", letter as char);
            if PathBuf::from(&drive_root).exists() {
                // TODO: Use windows crate for proper drive type detection
                drives.push(DriveInfo {
                    mount_point: PathBuf::from(drive_root),
                    total_space: 0,
                    available_space: 0,
                    filesystem: "NTFS".to_string(),
                    drive_type: DriveType::Fixed,
                });
            }
        }
        Ok(drives)
    }

    fn list_installed_apps(&self) -> Result<Vec<AppAnalysis>> {
        // TODO: Read from HKLM/HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall
        // For now, return empty — full implementation in Phase 2
        Ok(Vec::new())
    }

    fn get_app_last_launched(&self, _app_name: &str) -> Result<Option<SystemTime>> {
        // TODO: Check Prefetch directory and Event Logs
        Ok(None)
    }

    fn temp_directories(&self) -> Vec<PathBuf> {
        let mut temps = Vec::new();
        if let Ok(temp) = std::env::var("TEMP") {
            temps.push(PathBuf::from(temp));
        }
        if let Ok(tmp) = std::env::var("TMP") {
            temps.push(PathBuf::from(tmp));
        }
        // System temp
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            temps.push(PathBuf::from(format!("{}\\Temp", local_app_data)));
        }
        temps
    }

    fn cache_directories(&self) -> Vec<PathBuf> {
        let mut caches = Vec::new();
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            // Browser caches
            caches.push(PathBuf::from(format!(
                "{}\\Google\\Chrome\\User Data\\Default\\Cache",
                local_app_data
            )));
            caches.push(PathBuf::from(format!(
                "{}\\Microsoft\\Edge\\User Data\\Default\\Cache",
                local_app_data
            )));
            caches.push(PathBuf::from(format!(
                "{}\\Mozilla\\Firefox\\Profiles",
                local_app_data
            )));
            // Windows thumbnail cache
            caches.push(PathBuf::from(format!(
                "{}\\Microsoft\\Windows\\Explorer",
                local_app_data
            )));
        }
        caches
    }

    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }

    fn expand_env_vars(&self, path: &str) -> String {
        // Expand %VAR% style environment variables
        let mut result = path.to_string();
        let mut start = 0;
        while let Some(pos) = result[start..].find('%') {
            let abs_start = start + pos;
            if let Some(end) = result[abs_start + 1..].find('%') {
                let var_name = &result[abs_start + 1..abs_start + 1 + end];
                if let Ok(value) = std::env::var(var_name) {
                    result.replace_range(abs_start..abs_start + 2 + end, &value);
                    start = abs_start + value.len();
                } else {
                    start = abs_start + 1;
                }
            } else {
                break;
            }
        }
        result
    }

    fn is_system_path(&self, path: &PathBuf) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        path_str.contains("\\windows\\system32\\")
            || path_str.contains("\\windows\\syswow64\\")
            || path_str.ends_with("\\pagefile.sys")
            || path_str.ends_with("\\hiberfil.sys")
            || path_str.contains("\\recovery\\")
            || path_str.ends_with("\\ntuser.dat")
    }

    fn user_profile_dir(&self) -> Option<PathBuf> {
        self.home_dir()
    }

    fn uninstall_app(&self, _app_id: &str) -> Result<()> {
        // TODO: Use MSI API or run uninstaller executable
        todo!("Windows app uninstall implementation")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_expand_env_vars() {
        let platform = WindowsPlatform::new();
        // Test with a path that doesn't contain env vars
        assert_eq!(platform.expand_env_vars("C:\\Users\\test"), "C:\\Users\\test");
    }

    #[test]
    fn test_windows_is_system_path() {
        let platform = WindowsPlatform::new();
        assert!(platform.is_system_path(&PathBuf::from(
            "C:\\Windows\\System32\\kernel32.dll"
        )));
        assert!(platform.is_system_path(&PathBuf::from("C:\\pagefile.sys")));
        assert!(!platform.is_system_path(&PathBuf::from(
            "C:\\Users\\test\\Documents"
        )));
    }
}
