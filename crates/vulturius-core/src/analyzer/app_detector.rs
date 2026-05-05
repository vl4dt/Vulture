use std::path::PathBuf;
use std::time::SystemTime;

use tracing::info;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::types::*;
use crate::VulturiusError;

/// Application detector — finds installed apps and determines usage patterns.
pub struct AppDetector {
    config: AppConfig,
}

impl AppDetector {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    /// Detect all installed applications on the current platform.
    pub fn detect_apps(&self) -> Result<Vec<AppAnalysis>, VulturiusError> {
        info!("Detecting installed applications...");
        let mut apps = Vec::new();

        // Platform-specific detection
        apps.extend(self.detect_in_directories());

        // Post-process: compute usage scores and recommendations
        for app in &mut apps {
            app.usage_score = self.compute_usage_score(app);
            app.recommendation = self.compute_recommendation(app);
        }

        info!("Detected {} applications", apps.len());
        Ok(apps)
    }

    /// Detect apps by scanning known installation directories.
    fn detect_in_directories(&self) -> Vec<AppAnalysis> {
        let mut apps = Vec::new();
        let search_dirs = self.get_app_search_dirs();

        for dir in &search_dirs {
            if !dir.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let app = self.try_detect_app(&path);
                    if let Some(app) = app {
                        apps.push(app);
                    }
                }
            }
        }

        apps
    }

    /// Try to detect if a path is an installed application.
    fn try_detect_app(&self, path: &PathBuf) -> Option<AppAnalysis> {
        if !path.is_dir() {
            return None;
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())?;

        // Skip hidden directories
        if name.starts_with('.') {
            return None;
        }

        // Check for app indicators
        let is_app = self.check_app_indicators(path);
        if !is_app {
            return None;
        }

        let size = Self::dir_size(path);
        let last_modified = path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok());

        Some(AppAnalysis {
            id: Uuid::new_v4().to_string(),
            name,
            version: None,
            install_path: path.clone(),
            last_launched: last_modified,
            launch_count: 0,
            total_size: size,
            usage_score: 0.0,
            uninstall_safe: true,
            leftover_files: Vec::new(),
            registry_entries: Vec::new(),
            recommendation: AppRecommendation::Active,
        })
    }

    /// Check if a directory looks like an installed application.
    fn check_app_indicators(&self, path: &PathBuf) -> bool {
        // Look for common app indicator files
        let indicators = [
            "uninstall.exe",
            "Uninstall.exe",
            "UNINSTALL.EXE",
            "Info.plist",
            ".app",
            "setup.exe",
            "version.txt",
            "VERSION",
            "package.json",
            "Cargo.toml",
            "manifest.json",
        ];

        if !path.exists() {
            return false;
        }

        // Check if path itself ends with .app (macOS)
        if path.extension().map_or(false, |ext| ext == "app") {
            return true;
        }

        // Check for indicator files
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_name = entry
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase();
                for indicator in &indicators {
                    if file_name == indicator.to_lowercase() {
                        return true;
                    }
                }
            }
        }

        // Check for executable files in the directory
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if ext_lower == "exe" || ext_lower == "app" || ext_lower == "bin" {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get platform-specific app search directories.
    fn get_app_search_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Ok(program_files) = std::env::var("ProgramFiles") {
                dirs.push(PathBuf::from(program_files));
            }
            if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
                dirs.push(PathBuf::from(program_files_x86));
            }
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                dirs.push(PathBuf::from(local_app_data));
            }
            if let Ok(app_data) = std::env::var("APPDATA") {
                dirs.push(PathBuf::from(app_data));
            }
        }

        #[cfg(target_os = "macos")]
        {
            dirs.push(PathBuf::from("/Applications"));
            if let Ok(home) = std::env::var("HOME") {
                dirs.push(PathBuf::from(format!("{}/Applications", home)));
            }
        }

        #[cfg(target_os = "linux")]
        {
            dirs.push(PathBuf::from("/usr/bin"));
            dirs.push(PathBuf::from("/usr/local/bin"));
            dirs.push(PathBuf::from("/opt"));
            if let Ok(home) = std::env::var("HOME") {
                dirs.push(PathBuf::from(format!("{}/.local/bin", home)));
            }
        }

        // Fallback for unknown platforms
        if dirs.is_empty() {
            if let Ok(home) = std::env::var("HOME") {
                dirs.push(PathBuf::from(home));
            }
        }

        dirs
    }

    /// Compute a usage score (0.0 = never used, 1.0 = used daily).
    fn compute_usage_score(&self, app: &AppAnalysis) -> f64 {
        let now = SystemTime::now();

        if let Some(last_launched) = app.last_launched {
            let days_since = now
                .duration_since(last_launched)
                .map(|d| d.as_secs() as f64 / 86_400.0)
                .unwrap_or(365.0);

            // Exponential decay: half-life of ~90 days
            // score = e^(-ln(2) * days / 90)
            let score = (-0.0077 * days_since).exp();
            score.clamp(0.0, 1.0)
        } else {
            // Never launched
            0.0
        }
    }

    /// Compute app recommendation based on usage.
    fn compute_recommendation(&self, app: &AppAnalysis) -> AppRecommendation {
        let threshold = self.config.analysis.unused_app_threshold_days as f64;

        if app.usage_score > 0.8 {
            AppRecommendation::Active
        } else if app.usage_score > 0.3 {
            AppRecommendation::RarelyUsed
        } else if app.usage_score == 0.0 {
            AppRecommendation::Unused
        } else {
            // Check if it's a system/essential app
            if self.is_essential_app(&app.name) {
                AppRecommendation::Essential
            } else {
                AppRecommendation::Unused
            }
        }
    }

    /// Check if an app name suggests it's essential/system.
    fn is_essential_app(&self, name: &str) -> bool {
        let essential_names = [
            "windows",
            "system",
            "microsoft",
            "intel",
            "nvidia",
            "amd",
            "driver",
            "firmware",
            "security",
            "antivirus",
            "defender",
            "firewall",
        ];

        let name_lower = name.to_lowercase();
        essential_names.iter().any(|e| name_lower.contains(e))
    }

    /// Recursively compute directory size.
    fn dir_size(path: &PathBuf) -> u64 {
        walkdir::WalkDir::new(path)
            .max_depth(3) // Limit depth to avoid slow scans
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_essential_app() {
        let config = AppConfig::default();
        let detector = AppDetector::new(config);

        assert!(detector.is_essential_app("Windows Defender"));
        assert!(detector.is_essential_app("NVIDIA Driver"));
        assert!(detector.is_essential_app("Intel Graphics"));
        assert!(!detector.is_essential_app("Google Chrome"));
        assert!(!detector.is_essential_app("Visual Studio Code"));
        assert!(!detector.is_essential_app("Steam"));
    }

    #[test]
    fn test_compute_usage_score_recent() {
        let config = AppConfig::default();
        let detector = AppDetector::new(config);

        let app = AppAnalysis {
            id: "test".to_string(),
            name: "TestApp".to_string(),
            version: None,
            install_path: PathBuf::from("/test"),
            last_launched: Some(SystemTime::now()),
            launch_count: 10,
            total_size: 1024,
            usage_score: 0.0,
            uninstall_safe: true,
            leftover_files: Vec::new(),
            registry_entries: Vec::new(),
            recommendation: AppRecommendation::Active,
        };

        let score = detector.compute_usage_score(&app);
        assert!(score > 0.9); // Just launched = high score
    }

    #[test]
    fn test_compute_usage_score_old() {
        let config = AppConfig::default();
        let detector = AppDetector::new(config);

        let old_time = SystemTime::now() - std::time::Duration::from_secs(86_400 * 400);
        let app = AppAnalysis {
            id: "test".to_string(),
            name: "OldApp".to_string(),
            version: None,
            install_path: PathBuf::from("/test"),
            last_launched: Some(old_time),
            launch_count: 1,
            total_size: 1024,
            usage_score: 0.0,
            uninstall_safe: true,
            leftover_files: Vec::new(),
            registry_entries: Vec::new(),
            recommendation: AppRecommendation::Active,
        };

        let score = detector.compute_usage_score(&app);
        assert!(score < 0.3); // 400 days ago = low score
    }

    #[test]
    fn test_compute_usage_score_never_launched() {
        let config = AppConfig::default();
        let detector = AppDetector::new(config);

        let app = AppAnalysis {
            id: "test".to_string(),
            name: "NeverUsed".to_string(),
            version: None,
            install_path: PathBuf::from("/test"),
            last_launched: None,
            launch_count: 0,
            total_size: 1024,
            usage_score: 0.0,
            uninstall_safe: true,
            leftover_files: Vec::new(),
            registry_entries: Vec::new(),
            recommendation: AppRecommendation::Active,
        };

        let score = detector.compute_usage_score(&app);
        assert_eq!(score, 0.0);
    }
}
