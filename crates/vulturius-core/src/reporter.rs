//! Reporting engine for scan/analysis/cleanup data.
//!
//! Generates text, HTML, and JSON reports with trend analysis.

mod html_report;
mod json_report;
mod text_report;
mod trend;

pub use html_report::HtmlReportBuilder;
pub use json_report::JsonReport;
pub use text_report::TextReportBuilder;
pub use trend::{TrendAnalyzer, TrendSummary};

use crate::db::Database;
use crate::VulturiusError;
use std::path::PathBuf;
use tokio::fs;
use tracing::info;

/// Report format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Html,
    Json,
}

impl ReportFormat {
    pub fn extension(&self) -> &str {
        match self {
            ReportFormat::Text => "txt",
            ReportFormat::Html => "html",
            ReportFormat::Json => "json",
        }
    }
}

/// Report configuration.
#[derive(Debug, Clone)]
pub struct ReportConfig {
    /// Include scan details.
    pub include_scan: bool,
    /// Include analysis details.
    pub include_analysis: bool,
    /// Include cleanup history.
    pub include_cleanup: bool,
    /// Include app registry.
    pub include_apps: bool,
    /// Include quarantine status.
    pub include_quarantine: bool,
    /// Include trend analysis.
    pub include_trends: bool,
    /// Maximum entries per section.
    pub max_entries: usize,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            include_scan: true,
            include_analysis: true,
            include_cleanup: true,
            include_apps: true,
            include_quarantine: true,
            include_trends: true,
            max_entries: 100,
        }
    }
}

/// Generates reports from scan/analysis/cleanup data.
pub struct Reporter {
    db: Database,
    config: ReportConfig,
}

impl Reporter {
    /// Create a new reporter with default config.
    pub fn new(db: Database) -> Self {
        Self {
            db,
            config: ReportConfig::default(),
        }
    }

    /// Create with custom config.
    pub fn with_config(db: Database, config: ReportConfig) -> Self {
        Self { db, config }
    }

    /// Generate a text report.
    pub async fn generate_text(&self) -> Result<String, VulturiusError> {
        TextReportBuilder::new(&self.db, &self.config).build().await
    }

    /// Generate an HTML report.
    pub async fn generate_html(&self) -> Result<String, VulturiusError> {
        HtmlReportBuilder::new(&self.db, &self.config).build().await
    }

    /// Generate a JSON report.
    pub async fn generate_json(&self) -> Result<String, VulturiusError> {
        let report = JsonReport::from_db(&self.db, &self.config).await?;
        Ok(serde_json::to_string_pretty(&report)?)
    }

    /// Generate trend analysis.
    pub async fn analyze_trends(&self) -> Result<TrendSummary, VulturiusError> {
        TrendAnalyzer::new(&self.db).analyze().await
    }

    /// Save a report to disk.
    pub async fn save_report(&self, format: ReportFormat, path: PathBuf) -> Result<(), VulturiusError> {
        let content = match format {
            ReportFormat::Text => self.generate_text().await?,
            ReportFormat::Html => self.generate_html().await?,
            ReportFormat::Json => self.generate_json().await?,
        };

        fs::write(&path, &content).await?;
        info!("Report saved to {}", path.display());
        Ok(())
    }

    /// Generate all report formats and save to directory.
    pub async fn save_all_reports(&self, output_dir: PathBuf) -> Result<Vec<PathBuf>, VulturiusError> {
        fs::create_dir_all(&output_dir).await?;
        let mut saved = Vec::new();

        for format in &[ReportFormat::Text, ReportFormat::Html, ReportFormat::Json] {
            let path = output_dir.join(format!("vulturius_report.{0}", format.extension()));
            self.save_report(format.clone(), path.clone()).await?;
            saved.push(path);
        }

        Ok(saved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_report_format_extension() {
        assert_eq!(ReportFormat::Text.extension(), "txt");
        assert_eq!(ReportFormat::Html.extension(), "html");
        assert_eq!(ReportFormat::Json.extension(), "json");
    }

    #[test]
    fn test_report_config_default() {
        let config = ReportConfig::default();
        assert!(config.include_scan);
        assert!(config.include_analysis);
        assert!(config.include_cleanup);
        assert!(config.include_apps);
        assert!(config.include_quarantine);
        assert!(config.include_trends);
        assert_eq!(config.max_entries, 100);
    }

    #[test]
    fn test_format_size() {
        use text_report::format_size;
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1500), "1.50 KB");
        assert_eq!(format_size(1_500_000), "1.50 MB");
        assert_eq!(format_size(1_500_000_000), "1.50 GB");
        assert_eq!(format_size(1_500_000_000_000), "1.50 TB");
    }

    #[test]
    fn test_json_report_empty() {
        let report = JsonReport {
            title: "Test Report".to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            scan_summary: None,
            app_summary: None,
            cleanup_history: Vec::new(),
            quarantine: None,
            trends: None,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("Test Report"));
        // Empty fields are skipped via skip_serializing_if
        assert!(!json.contains("cleanup_history"));
    }
}
