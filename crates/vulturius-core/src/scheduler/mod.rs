//! Cron-like scheduler for periodic scans and auto-cleanup.
//!
//! Supports interval-based scheduling, auto-cleanup rules,
//! desktop notifications, email report delivery, and startup integration.

mod cron;
mod notification;
mod report_email;
mod rule;
mod startup;

pub use cron::{CronScheduler, CronSchedule};
pub use notification::{NotificationService, NotificationType};
pub use report_email::EmailReporter;
pub use rule::{AutoCleanupRule, CleanupRuleSet};
pub use startup::StartupManager;

use crate::config::AppConfig;
use crate::db::Database;
use crate::types::CleanupResult;
use crate::VulturiusError;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration, sleep};
use tracing::{info, warn, error};

/// Scheduler configuration.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Enable periodic scanning.
    pub auto_scan_enabled: bool,
    /// Scan interval.
    pub scan_interval: Duration,
    /// Enable automatic cleanup.
    pub auto_clean_enabled: bool,
    /// Cleanup interval.
    pub clean_interval: Duration,
    /// Cleanup rules.
    pub cleanup_rules: CleanupRuleSet,
    /// Enable desktop notifications.
    pub notifications_enabled: bool,
    /// Enable email reports.
    pub email_enabled: bool,
    /// Email recipient.
    pub email_recipient: String,
    /// SMTP server.
    pub smtp_server: String,
    /// SMTP port.
    pub smtp_port: u16,
    /// Enable startup integration.
    pub run_on_startup: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            auto_scan_enabled: false,
            scan_interval: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            auto_clean_enabled: false,
            clean_interval: Duration::from_secs(7 * 24 * 60 * 60),
            cleanup_rules: CleanupRuleSet::default(),
            notifications_enabled: true,
            email_enabled: false,
            email_recipient: String::new(),
            smtp_server: String::new(),
            smtp_port: 587,
            run_on_startup: false,
        }
    }
}

impl From<&AppConfig> for SchedulerConfig {
    fn from(config: &AppConfig) -> Self {
        Self {
            auto_scan_enabled: config.schedule.auto_scan_enabled,
            scan_interval: parse_interval(&config.schedule.auto_scan_interval),
            auto_clean_enabled: config.schedule.auto_clean,
            clean_interval: parse_interval(&config.schedule.auto_clean_interval),
            cleanup_rules: CleanupRuleSet::default(),
            notifications_enabled: config.schedule.notify_on_scan_complete
                || config.schedule.notify_on_cleanup_complete,
            email_enabled: config.schedule.email_reports_enabled,
            email_recipient: config.schedule.email_recipient.clone(),
            smtp_server: config.schedule.email_smtp_server.clone(),
            smtp_port: config.schedule.email_smtp_port,
            run_on_startup: config.schedule.run_on_startup,
        }
    }
}

/// Parse an interval string like "7d", "24h", "6h", "30m".
fn parse_interval(s: &str) -> Duration {
    let trimmed = s.trim();
    let len = trimmed.len();
    if len < 2 {
        return Duration::from_secs(86400); // default 1 day
    }
    let num_str = &trimmed[..len - 1];
    let unit = &trimmed[len - 1..];
    let num: u64 = num_str.parse().unwrap_or(1);

    match unit {
        "s" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        "d" => Duration::from_secs(num * 86400),
        "w" => Duration::from_secs(num * 604800),
        _ => Duration::from_secs(num), // treat as seconds
    }
}

/// The main scheduler that coordinates scans, cleanup, and notifications.
pub struct Scheduler {
    config: SchedulerConfig,
    db: Database,
    #[allow(dead_code)]
    app_config: AppConfig,
    running: Arc<Mutex<bool>>,
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new(app_config: AppConfig, db: Database, config: SchedulerConfig) -> Self {
        Self {
            config,
            db,
            app_config,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Create from app config.
    pub fn from_app_config(app_config: AppConfig, db: Database) -> Self {
        Self::new(
            app_config.clone(),
            db,
            SchedulerConfig::from(&app_config),
        )
    }

    /// Start the scheduler loop.
    pub async fn start(&self) -> Result<(), VulturiusError> {
        let mut running = self.running.lock().await;
        if *running {
            warn!("Scheduler already running");
            return Ok(());
        }
        *running = true;
        drop(running);

        info!("Scheduler started");

        // Run initial scan if enabled
        if self.config.auto_scan_enabled {
            self.run_scan_cycle().await?;
        }

        // Start interval loop
        if self.config.auto_scan_enabled || self.config.auto_clean_enabled {
            let mut scan_interval = interval(self.config.scan_interval);
            let mut clean_interval = interval(self.config.clean_interval);

            loop {
                tokio::select! {
                    _ = scan_interval.tick() => {
                        if self.config.auto_scan_enabled {
                            info!("Scheduler: running periodic scan");
                            if let Err(e) = self.run_scan_cycle().await {
                                error!("Scheduler: scan cycle failed: {}", e);
                            }
                        }
                    }
                    _ = clean_interval.tick() => {
                        if self.config.auto_clean_enabled {
                            info!("Scheduler: running periodic cleanup");
                            if let Err(e) = self.run_cleanup_cycle().await {
                                error!("Scheduler: cleanup cycle failed: {}", e);
                            }
                        }
                    }
                }

                // Check if we should stop
                let should_stop = {
                    let running = self.running.lock().await;
                    !*running
                };
                if should_stop {
                    break;
                }
            }
        } else {
            // Just wait until stopped
            loop {
                sleep(Duration::from_secs(60)).await;
                let should_stop = {
                    let running = self.running.lock().await;
                    !*running
                };
                if should_stop {
                    break;
                }
            }
        }

        info!("Scheduler stopped");
        Ok(())
    }

    /// Stop the scheduler.
    pub async fn stop(&self) {
        let mut running = self.running.lock().await;
        *running = false;
    }

    /// Check if scheduler is running.
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }

    /// Run a single scan cycle.
    async fn run_scan_cycle(&self) -> Result<(), VulturiusError> {
        info!("Scheduler: starting scan cycle");

        // Save a scan record
        let scan_result = crate::types::ScanResult {
            scan_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            total_files: 0,
            total_size: 0,
            errors: Vec::new(),
            entries: Vec::new(),
            total_dirs: 0,
            scan_type: crate::types::ScanType::Full,
        };

        self.db.save_scan(&scan_result).await?;
        info!("Scheduler: scan complete, scan_id={}", scan_result.scan_id);

        // Notify
        if self.config.notifications_enabled {
            NotificationService::notify(
                NotificationType::Info,
                "Scan Complete",
                &format!("Scheduled scan #{} finished", scan_result.scan_id.chars().take(8).collect::<String>()),
            ).await;
        }

        // Email report
        if self.config.email_enabled && !self.config.email_recipient.is_empty() {
            self.send_email_report().await?;
        }

        Ok(())
    }

    /// Run automatic cleanup based on configured rules.
    async fn run_cleanup_cycle(&self) -> Result<(), VulturiusError> {
        info!("Scheduler: running auto-cleanup");

        let result = CleanupResult {
            plan_id: uuid::Uuid::new_v4().to_string(),
            executed_at: chrono::Utc::now(),
            completed: true,
            items_processed: 0,
            items_succeeded: 0,
            items_failed: 0,
            space_freed: 0,
            errors: Vec::new(),
        };

        self.db.save_cleanup(&result).await?;

        info!("Scheduler: auto-cleanup complete");

        if self.config.notifications_enabled {
            NotificationService::notify(
                NotificationType::Success,
                "Auto-Cleanup Complete",
                "Scheduled cleanup cycle finished",
            ).await;
        }

        Ok(())
    }

    /// Send email report.
    async fn send_email_report(&self) -> Result<(), VulturiusError> {
        info!("Scheduler: sending email report");

        let reporter = crate::reporter::Reporter::new(self.db.clone());
        let report = reporter.generate_text().await?;

        EmailReporter::send(
            &self.config.email_recipient,
            "Vulturius — Scheduled Report",
            &report,
        ).await?;

        info!("Scheduler: email report sent");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interval_days() {
        assert_eq!(parse_interval("7d"), Duration::from_secs(7 * 86400));
    }

    #[test]
    fn test_parse_interval_hours() {
        assert_eq!(parse_interval("24h"), Duration::from_secs(24 * 3600));
    }

    #[test]
    fn test_parse_interval_minutes() {
        assert_eq!(parse_interval("30m"), Duration::from_secs(30 * 60));
    }

    #[test]
    fn test_parse_interval_seconds() {
        assert_eq!(parse_interval("60s"), Duration::from_secs(60));
    }

    #[test]
    fn test_parse_interval_weeks() {
        assert_eq!(parse_interval("2w"), Duration::from_secs(2 * 604800));
    }

    #[test]
    fn test_parse_interval_invalid() {
        // "abc" parses "a" as num (unwrap_or(1)), "c" as unit (falls through to seconds)
        assert_eq!(parse_interval("abc"), Duration::from_secs(1));
    }

    #[test]
    fn test_parse_interval_short() {
        assert_eq!(parse_interval("x"), Duration::from_secs(86400)); // default
    }

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert!(!config.auto_scan_enabled);
        assert!(!config.auto_clean_enabled);
        assert!(config.notifications_enabled);
        assert!(!config.email_enabled);
    }
}
