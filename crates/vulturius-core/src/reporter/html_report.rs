//! HTML report builder with inline SVG charts.

use crate::db::Database;
use crate::reporter::ReportConfig;
use crate::types::{AppRecommendation, CleanupResult, ScanResult};
use crate::VulturiusError;
use chrono::Utc;

/// Builds HTML reports with inline SVG visualizations.
pub struct HtmlReportBuilder<'a> {
    db: &'a Database,
    config: &'a ReportConfig,
}

impl<'a> HtmlReportBuilder<'a> {
    pub fn new(db: &'a Database, config: &'a ReportConfig) -> Self {
        Self { db, config }
    }

    /// Build the complete HTML report.
    pub async fn build(self) -> Result<String, VulturiusError> {
        let scan_id = self.db.get_latest_scan_id().await?;
        let scan_opt = if let Some(id) = scan_id {
            futures::executor::block_on(self.db.get_scan(&id)).unwrap_or(None)
        } else {
            None
        };
        let scan = scan_opt.unwrap_or_else(|| ScanResult {
            scan_id: "N/A".to_string(),
            started_at: Utc::now(),
            completed_at: Utc::now(),
            total_files: 0,
            total_size: 0,
            errors: Vec::new(),
            entries: Vec::new(),
            total_dirs: 0,
            scan_type: crate::types::ScanType::Full,
        });

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

        let history = self.db.get_cleanup_history(self.config.max_entries).await?;
        let quarantine = self.db.get_quarantine_entries().await?;

        let total_freed: u64 = history.iter().map(|e| e.space_freed).sum();
        let total_items: usize = history.iter().map(|e| e.items_processed).sum();
        let quarantine_size: u64 = quarantine.iter().map(|e| e.size).sum();

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
            --purple: #b829ff;
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
        .container {{ max-width: 960px; margin: 0 auto; }}
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
        .metric-value.ai {{ color: var(--purple); }}
        .chart-container {{
            display: flex;
            justify-content: center;
            margin: 1rem 0;
        }}
        .footer {{
            text-align: center;
            color: var(--dim);
            font-size: 0.65rem;
            margin-top: 2rem;
            letter-spacing: 0.1em;
        }}
        .history-item {{
            padding: 0.5rem 0;
            border-bottom: 1px solid rgba(255,255,255,0.05);
            font-size: 0.8rem;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Vulturius</h1>
        <div class="subtitle">DISK ANALYSIS REPORT</div>

        <div class="panel">
            <h2>Scan Summary</h2>
            <div class="metric">
                <span class="metric-label">Scan ID</span>
                <span class="metric-value">{scan_id}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Files Scanned</span>
                <span class="metric-value">{total_files}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Data Scanned</span>
                <span class="metric-value">{total_size}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Errors</span>
                <span class="metric-value {error_class}">{error_count}</span>
            </div>
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
            <div class="chart-container">
                {apps_chart}
            </div>
        </div>

        <div class="panel">
            <h2>Cleanup History</h2>
            <div class="metric">
                <span class="metric-label">Total Runs</span>
                <span class="metric-value">{cleanup_runs}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Items Processed</span>
                <span class="metric-value">{cleanup_items}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Space Freed</span>
                <span class="metric-value safe">{cleanup_freed}</span>
            </div>
            <div class="chart-container">
                {cleanup_chart}
            </div>
        </div>

        <div class="panel">
            <h2>Quarantine</h2>
            <div class="metric">
                <span class="metric-label">Entries</span>
                <span class="metric-value">{quarantine_count}</span>
            </div>
            <div class="metric">
                <span class="metric-label">Total Size</span>
                <span class="metric-value">{quarantine_size}</span>
            </div>
        </div>

        <div class="footer">
            VULTURIUS — GENERATED {timestamp}
        </div>
    </div>
</body>
</html>"#,
            scan_id = scan.scan_id,
            total_files = scan.total_files,
            total_size = format_size(scan.total_size),
            error_count = scan.errors.len(),
            error_class = if scan.errors.is_empty() { "safe" } else { "warn" },
            total_apps = apps.len(),
            unused_apps = unused,
            rarely_apps = rarely,
            active_apps = active,
            apps_chart = self.apps_donut_svg(apps.len(), unused, rarely, active),
            cleanup_runs = history.len(),
            cleanup_items = total_items,
            cleanup_freed = format_size(total_freed),
            cleanup_chart = self.cleanup_bar_svg(&history),
            quarantine_count = quarantine.len(),
            quarantine_size = format_size(quarantine_size),
            timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );

        Ok(html)
    }

    /// Generate a donut SVG for app distribution.
    fn apps_donut_svg(&self, total: usize, unused: usize, rarely: usize, active: usize) -> String {
        if total == 0 {
            return String::from("<svg width='200' height='200'></svg>");
        }

        let segments = [
            (unused, "#ff2d55", "Unused"),
            (rarely, "#ffb800", "Rarely"),
            (active, "#00d4aa", "Active"),
        ];

        let mut paths = Vec::new();
        let mut start_angle = 0.0_f64;
        let center = 100.0_f64;
        let radius = 80.0_f64;

        for &(count, color, _label) in &segments {
            if count == 0 {
                continue;
            }
            let fraction = count as f64 / total as f64;
            let sweep_angle = fraction * 360.0;
            let end_angle = start_angle + sweep_angle;

            let start_rad = (start_angle - 90.0) * std::f64::consts::PI / 180.0;
            let end_rad = (end_angle - 90.0) * std::f64::consts::PI / 180.0;

            let x1 = center + radius * start_rad.cos();
            let y1 = center + radius * start_rad.sin();
            let x2 = center + radius * end_rad.cos();
            let y2 = center + radius * end_rad.sin();

            let large_arc = if sweep_angle > 180.0 { 1 } else { 0 };

            paths.push(format!(
                "<path d='M {} {} A {} {} 0 {} {} {} L {} {} Z' fill='{}'/>",
                x1, y1, radius, radius, large_arc, x2, y2, center, center, color
            ));

            start_angle = end_angle;
        }

        format!(
            "<svg width=\"200\" height=\"200\" viewBox=\"0 0 200 200\">\n{}\n<circle cx=\"100\" cy=\"100\" r=\"50\" fill=\"rgba(10,15,30,0.9)\"/>\n<text x=\"100\" y=\"105\" text-anchor=\"middle\" fill=\"#e9e9ff\" font-size=\"14\" font-family=\"monospace\">{}</text>\n</svg>",
            paths.join("\n                "),
            total
        )
    }

    /// Generate a bar chart SVG for cleanup history.
    fn cleanup_bar_svg(&self, history: &[CleanupResult]) -> String {
        if history.is_empty() {
            return String::from("<svg width='400' height='150'></svg>");
        }

        let max_size = history.iter().map(|e| e.space_freed).max().unwrap_or(1);
        let bar_width = 30;
        let gap = 10;
        let max_height = 100;
        let width = history.len() * (bar_width + gap);

        let mut bars = Vec::new();
        for (i, entry) in history.iter().enumerate() {
            let height = (entry.space_freed as f64 / max_size as f64 * max_height as f64) as u32;
            let x = i * (bar_width + gap);
            let y = max_height - height;
            bars.push(format!(
                "<rect x='{}' y='{}' width='{}' height='{}' fill='#00d4aa' rx='2'/>",
                x, y, bar_width, height
            ));
        }

        format!(
            "<svg width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n{}\n</svg>",
            width,
            max_height + 20,
            width,
            max_height + 20,
            bars.join("\n                ")
        )
    }
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
