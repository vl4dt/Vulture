//! Cron-like schedule parser and executor.
//!
//! Supports interval-based scheduling with human-readable strings
//! like "7d", "24h", "6h", "30m".

use std::time::Duration;

/// Cron-like schedule definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronSchedule {
    /// Run every N seconds.
    Every(Duration),
    /// Run at specific times (hour, minute) each day.
    Daily { hour: u32, minute: u32 },
    /// Run on specific days of week at specific time.
    Weekly { day: Weekday, hour: u32, minute: u32 },
}

/// Days of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// Cron scheduler that manages scheduled tasks.
pub struct CronScheduler {
    schedule: CronSchedule,
    enabled: bool,
}

impl CronScheduler {
    /// Create a new cron scheduler with interval-based schedule.
    pub fn every(duration: Duration) -> Self {
        Self {
            schedule: CronSchedule::Every(duration),
            enabled: true,
        }
    }

    /// Create a daily scheduler.
    pub fn daily(hour: u32, minute: u32) -> Self {
        Self {
            schedule: CronSchedule::Daily { hour, minute },
            enabled: true,
        }
    }

    /// Create a weekly scheduler.
    pub fn weekly(day: Weekday, hour: u32, minute: u32) -> Self {
        Self {
            schedule: CronSchedule::Weekly { day, hour, minute },
            enabled: true,
        }
    }

    /// Parse a schedule string like "7d", "24h", "6h", "30m".
    pub fn parse_interval(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        let len = trimmed.len();
        if len < 2 {
            return None;
        }

        let num_str = &trimmed[..len - 1];
        let unit = &trimmed[len - 1..];
        let num: u64 = num_str.parse().ok()?;

        let duration = match unit {
            "s" => Duration::from_secs(num),
            "m" => Duration::from_secs(num * 60),
            "h" => Duration::from_secs(num * 3600),
            "d" => Duration::from_secs(num * 86400),
            "w" => Duration::from_secs(num * 604800),
            _ => return None,
        };

        Some(Self::every(duration))
    }

    /// Get the next run interval.
    pub fn interval(&self) -> Duration {
        match &self.schedule {
            CronSchedule::Every(d) => *d,
            CronSchedule::Daily { .. } => Duration::from_secs(86400),
            CronSchedule::Weekly { .. } => Duration::from_secs(604800),
        }
    }

    /// Check if scheduler is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable/disable the scheduler.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interval_days() {
        let scheduler = CronScheduler::parse_interval("7d").unwrap();
        assert_eq!(scheduler.interval(), Duration::from_secs(7 * 86400));
    }

    #[test]
    fn test_parse_interval_hours() {
        let scheduler = CronScheduler::parse_interval("24h").unwrap();
        assert_eq!(scheduler.interval(), Duration::from_secs(24 * 3600));
    }

    #[test]
    fn test_parse_interval_minutes() {
        let scheduler = CronScheduler::parse_interval("30m").unwrap();
        assert_eq!(scheduler.interval(), Duration::from_secs(30 * 60));
    }

    #[test]
    fn test_parse_interval_invalid() {
        assert!(CronScheduler::parse_interval("abc").is_none());
    }

    #[test]
    fn test_daily_schedule() {
        let scheduler = CronScheduler::daily(3, 0);
        assert!(scheduler.is_enabled());
        assert_eq!(scheduler.interval(), Duration::from_secs(86400));
    }

    #[test]
    fn test_weekly_schedule() {
        let scheduler = CronScheduler::weekly(Weekday::Monday, 4, 30);
        assert!(scheduler.is_enabled());
        assert_eq!(scheduler.interval(), Duration::from_secs(604800));
    }

    #[test]
    fn test_toggle_enabled() {
        let mut scheduler = CronScheduler::every(Duration::from_secs(3600));
        assert!(scheduler.is_enabled());
        scheduler.set_enabled(false);
        assert!(!scheduler.is_enabled());
    }
}
