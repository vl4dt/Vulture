use std::path::PathBuf;
use std::time::SystemTime;

use crate::platform::PlatformOps;
use crate::types::{AppAnalysis, DriveInfo};
use crate::Result;

pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformOps for LinuxPlatform {
    fn list_drives(&self) -> Result<Vec<DriveInfo>> {
        let mut drives = Vec::new();
        // Parse /proc/mounts for mounted filesystems
        if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let device = parts[0];
                    let mount_point = parts[1];
                    let fs_type = parts[2];

                    // Skip virtual filesystems
                    if device.starts_with("proc")
                        || device.starts_with("sysfs")
                        || device.starts_with("devpts")
                        || device.starts_with("tmpfs")
                        || device.starts_with("cgroup")
                    {
                        continue;
                    }

                    drives.push(DriveInfo {
                        mount_point: PathBuf::from(mount_point.to_string()),
                        total_space: 0,
                        available_space: 0,
                        filesystem: fs_type.to_string(),
                        drive_type: if device.starts_with("/dev/sd") || device.starts_with("/dev/nvme")
                        {
                            crate::types::DriveType::Fixed
                        } else if device.starts_with("/dev/sr") {
                            crate::types::DriveType::CDROM
                        } else if device.starts_with("//") || device.starts_with("\\\\") {
                            crate::types::DriveType::Network
                        } else {
                            crate::types::DriveType::Unknown
                        },
                    });
                }
            }
        }
        Ok(drives)
    }

    fn list_installed_apps(&self) -> Result<Vec<AppAnalysis>> {
        // TODO: Query dpkg, pacman, rpm, snap, flatpak
        // For now, scan common binary directories
        Ok(Vec::new())
    }

    fn get_app_last_launched(&self, _app_name: &str) -> Result<Option<SystemTime>> {
        // TODO: Check ~/.local/share/recently-used.xbel or systemd journal
        Ok(None)
    }

    fn temp_directories(&self) -> Vec<PathBuf> {
        let mut temps = Vec::new();
        temps.push(PathBuf::from("/tmp"));
        temps.push(PathBuf::from("/var/tmp"));
        if let Ok(temp) = std::env::var("XDG_RUNTIME_DIR") {
            temps.push(PathBuf::from(temp));
        }
        temps
    }

    fn cache_directories(&self) -> Vec<PathBuf> {
        let mut caches = Vec::new();
        if let Ok(cache_home) = std::env::var("XDG_CACHE_HOME") {
            caches.push(PathBuf::from(cache_home));
        } else if let Ok(home) = std::env::var("HOME") {
            caches.push(PathBuf::from(format!("{}/.cache", home)));
        }
        // Package manager caches
        caches.push(PathBuf::from("/var/cache/apt/archives"));
        caches.push(PathBuf::from("/var/cache/pacman/pkg"));
        caches.push(PathBuf::from("/var/cache/yum"));
        caches.push(PathBuf::from("/var/cache/dnf"));
        caches
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
            || path_str.starts_with("/dev/")
    }

    fn user_profile_dir(&self) -> Option<PathBuf> {
        self.home_dir()
    }

    fn uninstall_app(&self, _app_id: &str) -> Result<()> {
        // TODO: Use dpkg -r, pacman -R, rpm -e, snap remove, flatpak uninstall
        todo!("Linux app uninstall implementation")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_is_system_path() {
        let platform = LinuxPlatform::new();
        assert!(platform.is_system_path(&PathBuf::from("/bin/bash")));
        assert!(platform.is_system_path(&PathBuf::from("/usr/lib/libc.so")));
        assert!(platform.is_system_path(&PathBuf::from("/proc/self")));
        assert!(platform.is_system_path(&PathBuf::from("/dev/sda1")));
        assert!(!platform.is_system_path(&PathBuf::from("/home/user/docs")));
        assert!(!platform.is_system_path(&PathBuf::from("/tmp/test")));
        assert!(!platform.is_system_path(&PathBuf::from("/var/cache/apt")));
    }

    #[test]
    fn test_linux_cache_directories() {
        let platform = LinuxPlatform::new();
        let caches = platform.cache_directories();
        assert!(!caches.is_empty());
        assert!(caches.iter().any(|c| c.to_string_lossy().contains(".cache")));
    }
}
