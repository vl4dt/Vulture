//! Startup integration — register/unregister Vulturius to run on system boot.
//!
//! Platform-specific:
//! - Windows: Registry `Run` key
//! - macOS: LaunchAgent plist
//! - Linux: systemd user service

use crate::VulturiusError;
use std::process::Command;
use tracing::{info, warn};

/// Manages startup registration.
pub struct StartupManager;

impl StartupManager {
    /// Check if Vulturius is registered to run on startup.
    pub fn is_enabled() -> bool {
        #[cfg(target_os = "windows")]
        {
            Self::check_windows_startup()
        }
        #[cfg(target_os = "macos")]
        {
            Self::check_macos_startup()
        }
        #[cfg(target_os = "linux")]
        {
            Self::check_linux_startup()
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            false
        }
    }

    /// Register Vulturius to run on startup.
    pub fn enable() -> Result<(), VulturiusError> {
        #[cfg(target_os = "windows")]
        {
            Self::enable_windows_startup()
        }
        #[cfg(target_os = "macos")]
        {
            Self::enable_macos_startup()
        }
        #[cfg(target_os = "linux")]
        {
            Self::enable_linux_startup()
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            warn!("Startup registration not supported on this platform");
            Ok(())
        }
    }

    /// Unregister Vulturius from startup.
    pub fn disable() -> Result<(), VulturiusError> {
        #[cfg(target_os = "windows")]
        {
            Self::disable_windows_startup()
        }
        #[cfg(target_os = "macos")]
        {
            Self::disable_macos_startup()
        }
        #[cfg(target_os = "linux")]
        {
            Self::disable_linux_startup()
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            warn!("Startup deregistration not supported on this platform");
            Ok(())
        }
    }

    // ── Windows ──

    #[cfg(target_os = "windows")]
    fn check_windows_startup() -> bool {
        let output = Command::new("reg")
            .args([
                "query",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "Vulturius",
            ])
            .output();

        matches!(output, Ok(out) if out.status.success())
    }

    #[cfg(target_os = "windows")]
    fn enable_windows_startup() -> Result<(), VulturiusError> {
        let exe_path = std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let output = Command::new("reg")
            .args([
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "Vulturius",
                "/t",
                "REG_SZ",
                "/d",
                &exe_path,
                "/f",
            ])
            .output()?;

        if output.status.success() {
            info!("Registered Vulturius for Windows startup");
            Ok(())
        } else {
            warn!("Failed to register for Windows startup");
            Ok(())
        }
    }

    #[cfg(target_os = "windows")]
    fn disable_windows_startup() -> Result<(), VulturiusError> {
        let output = Command::new("reg")
            .args([
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "Vulturius",
                "/f",
            ])
            .output()?;

        if output.status.success() {
            info!("Unregistered Vulturius from Windows startup");
        }
        Ok(())
    }

    // ── macOS ──

    #[cfg(target_os = "macos")]
    fn check_macos_startup() -> bool {
        use std::path::Path;
        let home = std::env::var("HOME").unwrap_or_default();
        let plist_path = format!("{}/Library/LaunchAgents/com.vulturius.agent.plist", home);
        Path::new(&plist_path).exists()
    }

    #[cfg(target_os = "macos")]
    fn enable_macos_startup() -> Result<(), VulturiusError> {
        let exe_path = std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let home = std::env::var("HOME").unwrap_or_default();
        let plist_dir = format!("{}/Library/LaunchAgents", home);
        let plist_path = format!("{}/com.vulturius.agent.plist", plist_dir);

        std::fs::create_dir_all(&plist_dir)?;

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.vulturius.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#,
            exe_path
        );

        std::fs::write(&plist_path, plist)?;
        info!("Registered Vulturius for macOS startup");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn disable_macos_startup() -> Result<(), VulturiusError> {
        let home = std::env::var("HOME").unwrap_or_default();
        let plist_path = format!("{}/Library/LaunchAgents/com.vulturius.agent.plist", home);
        if std::path::Path::new(&plist_path).exists() {
            std::fs::remove_file(&plist_path)?;
        }
        info!("Unregistered Vulturius from macOS startup");
        Ok(())
    }

    // ── Linux ──

    #[cfg(target_os = "linux")]
    fn check_linux_startup() -> bool {
        use std::path::Path;
        let home = std::env::var("HOME").unwrap_or_default();
        let service_path = format!("{}/.config/systemd/user/vulturius.service", home);
        Path::new(&service_path).exists()
    }

    #[cfg(target_os = "linux")]
    fn enable_linux_startup() -> Result<(), VulturiusError> {
        let exe_path = std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let home = std::env::var("HOME").unwrap_or_default();
        let service_dir = format!("{}/.config/systemd/user", home);
        let service_path = format!("{}/vulturius.service", service_dir);

        std::fs::create_dir_all(&service_dir)?;

        let service = format!(
            r#"[Unit]
Description=Vulturius Disk Cleanup Agent
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure

[Install]
WantedBy=default.target
"#,
            exe_path
        );

        std::fs::write(&service_path, service)?;

        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let _ = Command::new("systemctl")
            .args(["--user", "enable", "vulturius.service"])
            .status();

        info!("Registered Vulturius for Linux startup");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn disable_linux_startup() -> Result<(), VulturiusError> {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "vulturius.service"])
            .status();

        let home = std::env::var("HOME").unwrap_or_default();
        let service_path = format!("{}/.config/systemd/user/vulturius.service", home);
        if std::path::Path::new(&service_path).exists() {
            std::fs::remove_file(&service_path)?;
        }

        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();

        info!("Unregistered Vulturius from Linux startup");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_manager_exists() {
        let manager = StartupManager;
        let _ = manager; // just verify it compiles
    }

    #[test]
    fn test_is_enabled_returns_bool() {
        let result = StartupManager::is_enabled();
        // Just verify it returns a bool, value depends on system state
        let _: bool = result;
    }
}
