use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::RiskTolerance;

/// Main application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub scan: ScanConfig,
    pub analysis: AnalysisConfig,
    pub cleanup: CleanupConfig,
    pub llm: LlmConfig,
    pub schedule: ScheduleConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub language: String,
    pub output_format: String,
    pub log_level: String,
    pub max_threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub default_depth: String,
    pub default_exclusions: Vec<String>,
    pub scan_hidden: bool,
    pub scan_system: bool,
    pub collect_hashes: bool,
    pub max_scan_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub risk_tolerance: RiskTolerance,
    pub unused_app_threshold_days: u64,
    pub min_file_age_days: u64,
    pub llm_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    pub quarantine_enabled: bool,
    pub quarantine_path: PathBuf,
    pub auto_backup_before_uninstall: bool,
    pub confirm_threshold_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub max_calls_per_scan: usize,
    pub timeout_seconds: u64,
    pub cache_decisions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub auto_scan_enabled: bool,
    pub auto_scan_interval: String,
    pub auto_clean: bool,
    pub auto_clean_interval: String,
    pub notify_on_scan_complete: bool,
    pub notify_on_cleanup_complete: bool,
    pub email_reports_enabled: bool,
    pub email_recipient: String,
    pub email_smtp_server: String,
    pub email_smtp_port: u16,
    pub run_on_startup: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                language: "en".to_string(),
                output_format: "table".to_string(),
                log_level: "info".to_string(),
                max_threads: 0,
            },
            scan: ScanConfig {
                default_depth: "unlimited".to_string(),
                default_exclusions: vec![
                    "%SYSTEMROOT%".to_string(),
                    "%TEMP%\\System".to_string(),
                    "Program Files\\Windows*".to_string(),
                ],
                scan_hidden: false,
                scan_system: false,
                collect_hashes: false,
                max_scan_size: 0,
            },
            analysis: AnalysisConfig {
                risk_tolerance: RiskTolerance::Balanced,
                unused_app_threshold_days: 90,
                min_file_age_days: 30,
                llm_enabled: false,
            },
            cleanup: CleanupConfig {
                quarantine_enabled: true,
                quarantine_path: PathBuf::from("~/.local/share/vulturius/quarantine"),
                auto_backup_before_uninstall: true,
                confirm_threshold_mb: 1000,
            },
            llm: LlmConfig {
                enabled: false,
                provider: "openai".to_string(),
                model: "gpt-4o-mini".to_string(),
                api_key: String::new(),
                max_calls_per_scan: 50,
                timeout_seconds: 30,
                cache_decisions: true,
            },
            schedule: ScheduleConfig {
                auto_scan_enabled: false,
                auto_scan_interval: "7d".to_string(),
                auto_clean: false,
                auto_clean_interval: "7d".to_string(),
                notify_on_scan_complete: true,
                notify_on_cleanup_complete: true,
                email_reports_enabled: false,
                email_recipient: String::new(),
                email_smtp_server: String::new(),
                email_smtp_port: 587,
                run_on_startup: false,
            },
        }
    }
}
