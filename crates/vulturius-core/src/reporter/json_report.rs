//! JSON report for programmatic consumption.

use crate::db::Database;
use crate::reporter::ReportConfig;
use crate::types::*;
use crate::VulturiusError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Complete JSON report structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonReport {
    pub title: String,
    pub generated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_summary: Option<ScanSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_summary: Option<AppSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cleanup_history: Vec<CleanupEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarantine: Option<QuarantineSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trends: Option<TrendData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanSummary {
    pub scan_id: String,
    pub started_at: String,
    pub completed_at: String,
    pub duration_seconds: u64,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub error_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSummary {
    pub total: usize,
    pub unused: usize,
    pub rarely_used: usize,
    pub active: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CleanupEntry {
    pub executed_at: String,
    pub items_processed: usize,
    pub space_freed_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuarantineSummary {
    pub entry_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrendData {
    pub cleanup_runs: usize,
    pub avg_space_freed_bytes: u64,
    pub trend_direction: String,
}

impl JsonReport {
    /// Build a JSON report from the database.
    pub async fn from_db(db: &Database, config: &ReportConfig) -> Result<Self, VulturiusError> {
        let mut report = JsonReport {
            title: "Vulturius Disk Analysis Report".to_string(),
            generated_at: Utc::now().to_rfc3339(),
            scan_summary: None,
            app_summary: None,
            cleanup_history: Vec::new(),
            quarantine: None,
            trends: None,
        };

        if config.include_scan {
            report.scan_summary = Self::build_scan_summary(db).await?;
        }

        if config.include_apps {
            report.app_summary = Self::build_app_summary(db).await?;
        }

        if config.include_cleanup {
            report.cleanup_history = Self::build_cleanup_history(db, config).await?;
        }

        if config.include_quarantine {
            report.quarantine = Self::build_quarantine_summary(db).await?;
        }

        if config.include_trends {
            report.trends = Self::build_trends(db, config).await?;
        }

        Ok(report)
    }

    async fn build_scan_summary(db: &Database) -> Result<Option<ScanSummary>, VulturiusError> {
        let scan_id = db.get_latest_scan_id().await?;
        if let Some(id) = scan_id {
            if let Some(scan) = db.get_scan(&id).await? {
                let duration = scan
                    .completed_at
                    .signed_duration_since(scan.started_at)
                    .num_seconds()
                    .max(0) as u64;

                return Ok(Some(ScanSummary {
                    scan_id: scan.scan_id,
                    started_at: scan.started_at.to_rfc3339(),
                    completed_at: scan.completed_at.to_rfc3339(),
                    duration_seconds: duration,
                    total_files: scan.total_files,
                    total_size_bytes: scan.total_size,
                    error_count: scan.errors.len(),
                }));
            }
        }
        Ok(None)
    }

    async fn build_app_summary(db: &Database) -> Result<Option<AppSummary>, VulturiusError> {
        let apps = db.get_all_apps().await?;
        let unused = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::Unused)).count();
        let rarely = apps
            .iter()
            .filter(|a| matches!(a.recommendation, AppRecommendation::RarelyUsed))
            .count();
        let active = apps
            .iter()
            .filter(|a| matches!(a.recommendation, AppRecommendation::Active))
            .count();

        Ok(Some(AppSummary {
            total: apps.len(),
            unused,
            rarely_used: rarely,
            active,
        }))
    }

    async fn build_cleanup_history(
        db: &Database,
        config: &ReportConfig,
    ) -> Result<Vec<CleanupEntry>, VulturiusError> {
        let history = db.get_cleanup_history(config.max_entries).await?;
        Ok(history
            .into_iter()
            .map(|entry| CleanupEntry {
                executed_at: entry.executed_at.to_rfc3339(),
                items_processed: entry.items_processed,
                space_freed_bytes: entry.space_freed,
            })
            .collect())
    }

    async fn build_quarantine_summary(db: &Database) -> Result<Option<QuarantineSummary>, VulturiusError> {
        let entries = db.get_quarantine_entries().await?;
        if entries.is_empty() {
            return Ok(None);
        }
        let total_size: u64 = entries.iter().map(|e| e.size).sum();
        Ok(Some(QuarantineSummary {
            entry_count: entries.len(),
            total_size_bytes: total_size,
        }))
    }

    async fn build_trends(
        db: &Database,
        config: &ReportConfig,
    ) -> Result<Option<TrendData>, VulturiusError> {
        let history = db.get_cleanup_history(config.max_entries).await?;
        if history.len() < 2 {
            return Ok(None);
        }

        let total_freed: u64 = history.iter().map(|e| e.space_freed).sum();
        let avg = total_freed / history.len() as u64;

        let mid = history.len() / 2;
        let recent: u64 = history.iter().take(mid).map(|e| e.space_freed).sum();
        let older: u64 = history.iter().skip(mid).map(|e| e.space_freed).sum();

        let recent_f64 = recent as f64;
        let older_f64 = older as f64;
        let trend = if recent_f64 > older_f64 * 1.2 {
            "increasing"
        } else if recent_f64 < older_f64 * 0.8 {
            "decreasing"
        } else {
            "stable"
        };

        Ok(Some(TrendData {
            cleanup_runs: history.len(),
            avg_space_freed_bytes: avg,
            trend_direction: trend.to_string(),
        }))
    }
}
