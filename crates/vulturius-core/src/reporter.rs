use crate::config::AppConfig;
use crate::db::Database;
use crate::types::*;
use crate::VulturiusError;

use chrono::Utc;
use std::path::PathBuf;
use tokio::fs;
use tracing::info;

/// Generates reports from scan/analysis/cleanup data.
pub struct Reporter {
    db: Database,
}

impl Reporter {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Generate a text report.
    pub async fn generate_text_report(&self) -> Result<String, VulturiusError> {
        let mut report = String::new();
        report.push_str("╔══════════════════════════════════════════════════════════╗\n");
        report.push_str("║              VULTURIUS — DISK ANALYSIS REPORT            ║\n");
        report.push_str("╚══════════════════════════════════════════════════════════╝\n\n");

        // Latest scan info
        if let Some(scan_id) = self.db.get_latest_scan_id().await? {
            if let Some(scan) = self.db.get_scan(&scan_id).await? {
                report.push_str(&format!("Scan ID        : {}\n", scan.scan_id));
                report.push_str(&format!("Started        : {}\n", scan.started_at.format("%Y-%m-%d %H:%M:%S")));
                report.push_str(&format!("Completed      : {}\n", scan.completed_at.format("%Y-%m-%d %H:%M:%S")));
                report.push_str(&format!("Duration       : {:?}\n", scan.completed_at.signed_duration_since(scan.started_at)));
                report.push_str(&format!("Files scanned  : {}\n", scan.total_files));
                report.push_str(&format!("Data scanned   : {}\n", format_size(scan.total_size)));
                report.push_str(&format!("Errors         : {}\n\n", scan.errors.len()));
            }
        }

        // App registry summary
        let apps = self.db.get_all_apps().await?;
        let unused = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::Unused)).count();
        let rarely = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::RarelyUsed)).count();
        let active = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::Active)).count();

        report.push_str("── Application Registry ──────────────────────────────────\n");
        report.push_str(&format!("  Total apps     : {}\n", apps.len()));
        report.push_str(&format!("  Unused         : {}\n", unused));
        report.push_str(&format!("  Rarely used    : {}\n", rarely));
        report.push_str(&format!("  Active         : {}\n\n", active));

        // Cleanup history
        let history = self.db.get_cleanup_history(10).await?;
        if !history.is_empty() {
            report.push_str("── Recent Cleanup History ────────────────────────────────\n");
            for entry in &history {
                report.push_str(&format!(
                    "  {} — {} items, freed {}\n",
                    entry.executed_at.format("%Y-%m-%d %H:%M"),
                    entry.items_processed,
                    format_size(entry.space_freed),
                ));
            }
            report.push('\n');
        }

        // Quarantine status
        let quarantine = self.db.get_quarantine_entries().await?;
        if !quarantine.is_empty() {
            report.push_str("── Quarantine ────────────────────────────────────────────\n");
            report.push_str(&format!("  Entries        : {}\n", quarantine.len()));
            let total_q_size: u64 = quarantine.iter().map(|e| e.size).sum();
            report.push_str(&format!("  Total size     : {}\n", format_size(total_q_size)));
            report.push('\n');
        }

        report.push_str("── Generated: ────────────────────────────────────────────\n");
        report.push_str(&format!("  {}\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

        Ok(report)
    }

    /// Generate an HTML report.
    pub async fn generate_html_report(&self) -> Result<String, VulturiusError> {
        let text_report = self.generate_text_report().await?;
        let apps = self.db.get_all_apps().await?;
        let unused = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::Unused)).count();
        let rarely = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::RarelyUsed)).count();
        let active = apps.iter().filter(|a| matches!(a.recommendation, AppRecommendation::Active)).count();

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Vulturius — Disk Analysis Report</title>
    <style>
        :root {{
            --bg: #000008;
            --panel: rgba(10, 15, 30, 0.85);
            --cyan: #00f0ff;
            --teal: #00d4aa;
            --amber: #ffb800;
            --crimson: #ff2d55;
            --text: #e9e9ff;
            --dim: #666688;
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            background: var(--bg);
            color: var(--text);
            font-family: 'JetBrains Mono', 'IBM Plex Mono', 'Consolas', monospace;
            padding: 2rem;
            min-height: 100vh;
        }}
        .container {{
            max-width: 900px;
            margin: 0 auto;
        }}
        h1 {{
            color: var(--cyan);
            font-size: 1.5rem;
            letter-spacing: 0.3em;
            text-transform: uppercase;
            margin-bottom: 0.5rem;
            text-shadow: 0 0 20px rgba(0, 240, 255, 0.3);
        }}
        .subtitle {{
            color: var(--dim);
            font-size: 0.75rem;
            letter-spacing: 0.2em;
            margin-bottom: 2rem;
        }}
        .panel {{
            background: var(--panel);
            border: 1px solid rgba(0, 240, 255, 0.1);
            border-radius: 8px;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
            backdrop-filter: blur(10px);
        }}
        .panel h2 {{
            color: var(--teal);
            font-size: 0.85rem;
            letter-spacing: 0.15em;
            text-transform: uppercase;
            margin-bottom: 1rem;
            padding-bottom: 0.5rem;
            border-bottom: 1px solid rgba(0, 212, 170, 0.2);
        }}
        .metric {{
            display: flex;
            justify-content: space-between;
            padding: 0.4rem 0;
        }}
        .metric-label {{ color: var(--dim); }}
        .metric-value {{ color: var(--text); }}
        .metric-value.safe {{ color: var(--teal); }}
        .metric-value.warn {{ color: var(--amber); }}
        .metric-value.danger {{ color: var(--crimson); }}
        pre {{
            background: rgba(0, 0, 0, 0.5);
            padding: 1rem;
            border-radius: 4px;
            overflow-x: auto;
            font-size: 0.75rem;
            line-height: 1.6;
            color: var(--dim);
        }}
        .footer {{
            text-align: center;
            color: var(--dim);
            font-size: 0.65rem;
            margin-top: 2rem;
            letter-spacing: 0.1em;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Vulturius</h1>
        <div class="subtitle">DISK ANALYSIS REPORT</div>

        <div class="panel">
            <h2>Scan Summary</h2>
            {scan_metrics}
        </div>

        <div class="panel">
            <h2>Application Registry</h2>
            <div class="metric">
                <span class="metric-label">Total Apps</span>
                <span class="metric-value">{total_apps}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Unused</span>
                <span class="metric-value danger">{unused_apps}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Rarely Used</span>
                <span class="metric-value warn">{rarely_apps}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Active</span>
                <span class="metric-value safe">{active_apps}</span>
            </div>
        </div>

        <div class="panel">
            <h2>Full Report</h2>
            <pre>{report_text}</pre>
        </div>

        <div class="footer">
            VULTURIUS — GENERATED {timestamp}
        </div>
    </div>
</body>
</html>"#,
            scan_metrics = self.build_scan_metrics_html(),
            total_apps = apps.len(),
            unused_apps = unused,
            rarely_apps = rarely,
            active_apps = active,
            report_text = html_escape(&text_report),
            timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );

        Ok(html)
    }

    /// Save a report to disk.
    pub async fn save_report(&self, format: ReportFormat, path: PathBuf) -> Result<(), VulturiusError> {
        let content = match format {
            ReportFormat::Text => self.generate_text_report().await?,
            ReportFormat::Html => self.generate_html_report().await?,
        };

        fs::write(&path, &content).await?;
        info!("Report saved to {}", path.display());
        Ok(())
    }

    fn build_scan_metrics_html(&self) -> String {
        // This is a simplified version; in production, fetch from DB
        String::from(
            r#"<div class="metric">
                <span class="metric-label">Status</span>
                <span class="metric-value safe">Complete</span>
            </div>
            <div class="metric">
                <span class="metric-label">Files Scanned</span>
                <span class="metric-value">See full report</span>
            </div>"#,
        )
    }
}

/// Report format enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Html,
}

fn format_size(bytes: u64) -> String {
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
