//! Trend analysis for cleanup patterns.

use crate::db::Database;
use crate::VulturiusError;
use serde::{Deserialize, Serialize};

/// Trend analysis summary.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrendSummary {
    /// Number of cleanup runs analyzed.
    pub cleanup_runs: usize,
    /// Average space freed per run.
    pub avg_space_freed_bytes: u64,
    /// Total space freed across all runs.
    pub total_space_freed_bytes: u64,
    /// Trend direction.
    pub trend_direction: TrendDirection,
    /// Average items processed per run.
    pub avg_items_per_run: usize,
    /// Recent vs older comparison.
    pub recent_vs_older: RecentVsOlder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
    InsufficientData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecentVsOlder {
    pub recent_freed_bytes: u64,
    pub older_freed_bytes: u64,
    pub ratio: f64,
}

/// Analyzes trends in cleanup data.
pub struct TrendAnalyzer<'a> {
    db: &'a Database,
}

impl<'a> TrendAnalyzer<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Run the full trend analysis.
    pub async fn analyze(self) -> Result<TrendSummary, VulturiusError> {
        let history = self.db.get_cleanup_history(100).await?;

        if history.len() < 2 {
            return Ok(TrendSummary {
                cleanup_runs: history.len(),
                avg_space_freed_bytes: 0,
                total_space_freed_bytes: 0,
                trend_direction: TrendDirection::InsufficientData,
                avg_items_per_run: 0,
                recent_vs_older: RecentVsOlder {
                    recent_freed_bytes: 0,
                    older_freed_bytes: 0,
                    ratio: 0.0,
                },
            });
        }

        let total_freed: u64 = history.iter().map(|e| e.space_freed).sum();
        let total_items: usize = history.iter().map(|e| e.items_processed).sum();
        let avg_freed = total_freed / history.len() as u64;
        let avg_items = total_items / history.len();

        let mid = history.len() / 2;
        let recent_freed: u64 = history.iter().take(mid).map(|e| e.space_freed).sum();
        let older_freed: u64 = history.iter().skip(mid).map(|e| e.space_freed).sum();

        let ratio = if older_freed > 0 {
            recent_freed as f64 / older_freed as f64
        } else {
            0.0
        };

        let trend = if ratio > 1.2 {
            TrendDirection::Increasing
        } else if ratio < 0.8 {
            TrendDirection::Decreasing
        } else {
            TrendDirection::Stable
        };

        Ok(TrendSummary {
            cleanup_runs: history.len(),
            avg_space_freed_bytes: avg_freed,
            total_space_freed_bytes: total_freed,
            trend_direction: trend,
            avg_items_per_run: avg_items,
            recent_vs_older: RecentVsOlder {
                recent_freed_bytes: recent_freed,
                older_freed_bytes: older_freed,
                ratio,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_direction_display() {
        let increasing = TrendDirection::Increasing;
        let decreasing = TrendDirection::Decreasing;
        let stable = TrendDirection::Stable;
        let insufficient = TrendDirection::InsufficientData;

        assert_ne!(increasing, decreasing);
        assert_ne!(stable, insufficient);
        assert_eq!(increasing, TrendDirection::Increasing);
    }

    #[test]
    fn test_recent_vs_older_ratio() {
        let rvo = RecentVsOlder {
            recent_freed_bytes: 1000,
            older_freed_bytes: 500,
            ratio: 2.0,
        };
        assert_eq!(rvo.ratio, 2.0);
    }
}
