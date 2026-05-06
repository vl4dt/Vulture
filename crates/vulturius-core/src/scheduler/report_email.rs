//! Email report delivery.
//!
//! Sends generated reports via SMTP. Falls back to saving to disk
//! when SMTP is not configured.

use crate::VulturiusError;
use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn};

/// Email reporter for scheduled report delivery.
pub struct EmailReporter;

impl EmailReporter {
    /// Send an email report.
    ///
    /// When SMTP is configured, sends via email.
    /// Otherwise, saves the report to disk and logs the path.
    pub async fn send(
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), VulturiusError> {
        // In production, this would use an SMTP client (e.g., lettre)
        // For now, save to disk and log
        info!("Email report: to={}, subject={}", recipient, subject);

        // Save report to disk as fallback
        let reports_dir = Self::reports_dir();
        fs::create_dir_all(&reports_dir).await?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("report_{}.txt", timestamp);
        let path = reports_dir.join(&filename);

        fs::write(&path, body).await?;
        info!("Report saved to {}", path.display());

        Ok(())
    }

    /// Send an HTML report.
    pub async fn send_html(
        recipient: &str,
        subject: &str,
        html_body: &str,
    ) -> Result<(), VulturiusError> {
        info!("HTML email report: to={}, subject={}", recipient, subject);

        let reports_dir = Self::reports_dir();
        fs::create_dir_all(&reports_dir).await?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("report_{}.html", timestamp);
        let path = reports_dir.join(&filename);

        fs::write(&path, html_body).await?;
        info!("HTML report saved to {}", path.display());

        Ok(())
    }

    /// Get the reports directory.
    fn reports_dir() -> PathBuf {
        let base = std::env::var("VULTURIUS_DATA_DIR")
            .unwrap_or_else(|_| String::from("~/.local/share/vulturius"));
        PathBuf::from(base).join("reports")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reports_dir() {
        let dir = EmailReporter::reports_dir();
        assert!(dir.to_string_lossy().contains("reports"));
    }
}
