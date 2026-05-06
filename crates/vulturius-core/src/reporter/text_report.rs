//! Text report builder.

use crate::db::Database;
use crate::reporter::ReportConfig;
use crate::types::{AppRecommendation, FileAnalysis, FileCategory, Recommendation};
use crate::VulturiusError;
use chrono::Utc;

/// Builds text reports from database data.
pub struct TextReportBuilder<'a> {
    db: &'a Database,
    config: &'a ReportConfig,
}

impl<'a> TextReportBuilder<'a> {
    pub fn new(db: &'a Database, config: &'a ReportConfig) -> Self {
        Self { db, config }
    }

    /// Build the complete text report.
    pub async fn build(self) -> Result<String, VulturiusError> {
        let mut report = String::new();
        report.push_str(&self.header());

        if self.config.include_scan {
            report.push_str(&self.scan_section().await?);
        }

        if self.config.include_apps {
            report.push_str(&self.apps_section().await?);
        }

        if self.config.include_cleanup {
            report.push_str(&self.cleanup_section().await?);
        }

        if self.config.include_quarantine {
            report.push_str(&self.quarantine_section().await?);
        }

        if self.config.include_trends {
            report.push_str(&self.trend_section().await?);
        }

        report.push_str(&self.footer());
        Ok(report)
    }

    fn header(&self) -> String {
        format!(
            "╔══════════════════════════════════════════════════════════╗\n\
             ║              VULTURIUS — DISK ANALYSIS REPORT            ║\n\
             ╚══════════════════════════════════════════════════════════╝\n\n"
        )
    }

    async fn scan_section(&self) -> Result<String, VulturiusError> {
        let mut section = String::new();
        section.push_str("── Scan Summary ──────────────────────────────────────────\n");

        if let Some(scan_id) = self.db.get_latest_scan_id().await? {
            if let Some(scan) = self.db.get_scan(&scan_id).await? {
                let duration = scan.completed_at.signed_duration_since(scan.started_at);
                section.push_str(&format!(
                    "  Scan ID        : {}\n\
                     \tStarted        : {}\n\
                     \tCompleted      : {}\n\
                     \tDuration       : {:?}\n\
                     \tFiles scanned  : {}\n\
                     \tData scanned   : {}\n\
                     \tErrors         : {}\n",
                    scan.scan_id,
                    scan.started_at.format("%Y-%m-%d %H:%M:%S"),
                    scan.completed_at.format("%Y-%m-%d %H:%M:%S"),
                    duration,
                    scan.total_files,
                    format_size(scan.total_size),
                    scan.errors.len(),
                ));

                // Category breakdown (from analysis table)
                if self.config.include_analysis {
                    let analyses = self.db.get_analysis(&vec![]).await.unwrap_or_default();
                    section.push_str(&self.analysis_summary(&analyses));
                }
            }
        } else {
            section.push_str("  No scans found.\n");
        }

        section.push('\n');
        Ok(section)
    }

    fn analysis_summary(&self, analyses: &[FileAnalysis]) -> String {
        let mut breakdown = String::from("  ── Analysis Summary ──────────────────────────────\n");
        let safe = analyses.iter().filter(|a| matches!(a.recommendation, Recommendation::SafeToDelete)).count();
        let caution = analyses.iter().filter(|a| matches!(a.recommendation, Recommendation::Caution)).count();
        let do_not_delete = analyses.iter().filter(|a| matches!(a.recommendation, Recommendation::DoNotDelete)).count();
        let review = analyses.iter().filter(|a| matches!(a.recommendation, Recommendation::ReviewManually)).count();

        breakdown.push_str(&format!(
            "    Safe to delete : {}\n\tCaution        : {}\n\tDo not delete  : {}\n\tReview         : {}\n",
            safe,
            caution,
            do_not_delete,
            review,
        ));
        breakdown
    }

    async fn apps_section(&self) -> Result<String, VulturiusError> {
        let mut section = String::new();
        section.push_str("── Application Registry ──────────────────────────────────\n");

        let apps = self.db.get_all_apps().await?;
        let unused = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::Unused)).count();
        let rarely = apps
            .iter()
            .filter(|a| matches!(a.recommendation, AppRecommendation::RarelyUsed))
            .count();
        let active = apps
            .iter()
            .filter(|a| matches!(a.recommendation, AppRecommendation::Active))
            .count();

        section.push_str(&format!(
            "  Total apps     : {}\n\
             \tUnused         : {}\n\
             \tRarely used    : {}\n\
             \tActive         : {}\n",
            apps.len(),
            unused,
            rarely,
            active,
        ));

        // List unused apps
        if unused > 0 {
            section.push_str("  ── Unused Applications ─────────────────────────────\n");
            for app in apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::Unused)) {
                let last_launched = app
                    .last_launched
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        dt.format("%Y-%m-%d").to_string()
                    })
                    .unwrap_or_else(|| "N/A".to_string());

                section.push_str(&format!(
                    "    • {} ({} — {})\n",
                    app.name,
                    last_launched,
                    format_size(app.total_size),
                ));
            }
        }

        section.push('\n');
        Ok(section)
    }

    async fn cleanup_section(&self) -> Result<String, VulturiusError> {
        let mut section = String::new();
        section.push_str("── Cleanup History ───────────────────────────────────────\n");

        let history = self.db.get_cleanup_history(self.config.max_entries).await?;
        if history.is_empty() {
            section.push_str("  No cleanup history.\n");
        } else {
            for entry in &history {
                section.push_str(&format!(
                    "  {} — {} items, freed {}\n",
                    entry.executed_at.format("%Y-%m-%d %H:%M"),
                    entry.items_processed,
                    format_size(entry.space_freed),
                ));
            }

            let total_freed: u64 = history.iter().map(|e| e.space_freed).sum();
            let total_items: usize = history.iter().map(|e| e.items_processed).sum();
            section.push_str(&format!(
                "\n  Total: {} items, {} freed\n",
                total_items,
                format_size(total_freed),
            ));
        }

        section.push('\n');
        Ok(section)
    }

    async fn quarantine_section(&self) -> Result<String, VulturiusError> {
        let mut section = String::new();
        section.push_str("── Quarantine ────────────────────────────────────────────\n");

        let quarantine = self.db.get_quarantine_entries().await?;
        if quarantine.is_empty() {
            section.push_str("  Quarantine is empty.\n");
        } else {
            let total_size: u64 = quarantine.iter().map(|e| e.size).sum();
            section.push_str(&format!(
                "  Entries        : {}\n\
                 \tTotal size     : {}\n",
                quarantine.len(),
                format_size(total_size),
            ));
        }

        section.push('\n');
        Ok(section)
    }

    async fn trend_section(&self) -> Result<String, VulturiusError> {
        let mut section = String::new();
        section.push_str("── Trend Analysis ────────────────────────────────────────\n");

        // Simple trend: cleanup frequency and volume
        let history = self.db.get_cleanup_history(self.config.max_entries).await?;
        if history.len() >= 2 {
            let recent: Vec<_> = history.iter().take(history.len() / 2).collect();
            let older: Vec<_> = history.iter().skip(history.len() / 2).collect();

            let recent_freed: u64 = recent.iter().map(|e| e.space_freed).sum();
            let older_freed: u64 = older.iter().map(|e| e.space_freed).sum();

            let trend = if recent_freed > older_freed {
                "⚠ Increasing cleanup needs"
            } else if recent_freed < older_freed {
                "✓ Cleanup needs decreasing"
            } else {
                "→ Stable cleanup pattern"
            };

            section.push_str(&format!(
                "  Recent period  : {} freed\n\
                 \tOlder period   : {} freed\n\
                 \tTrend          : {}\n",
                format_size(recent_freed),
                format_size(older_freed),
                trend,
            ));
        } else {
            section.push_str("  Insufficient data for trend analysis.\n");
        }

        section.push('\n');
        Ok(section)
    }

    fn footer(&self) -> String {
        format!(
            "── Generated: ────────────────────────────────────────────\n\
             \t{}\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}

/// Format a file category for display.
#[allow(dead_code)]
pub fn format_category(cat: &FileCategory) -> String {
    match cat {
        FileCategory::Temp => "Temp",
        FileCategory::Cache => "Cache",
        FileCategory::Log => "Log",
        FileCategory::CrashDump => "Crash Dump",
        FileCategory::Thumbnail => "Thumbnail",
        FileCategory::DownloadHelper => "Download Helper",
        FileCategory::PackageCache => "Package Cache",
        FileCategory::BrowserData => "Browser Data",
        FileCategory::OldBackup => "Old Backup",
        FileCategory::Duplicate => "Duplicate",
        FileCategory::Orphaned => "Orphaned",
        FileCategory::Document => "Document",
        FileCategory::Image => "Image",
        FileCategory::Video => "Video",
        FileCategory::Audio => "Audio",
        FileCategory::Executable => "Executable",
        FileCategory::Library => "Library",
        FileCategory::Config => "Config",
        FileCategory::Unknown => "Unknown",
    }
    .to_string()
}

/// Format bytes to human-readable size.
pub fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000_000 {
        format!("{:.2} TB", bytes as f64 / 1_000_000_000_000.0)
    } else if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.2} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}
