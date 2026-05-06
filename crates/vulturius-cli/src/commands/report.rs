use vulturius_core::config::AppConfig;
use vulturius_core::db::Database;
use vulturius_core::reporter::{Reporter, ReportFormat};
use std::path::PathBuf;

pub async fn handle(_config: &AppConfig, format: String, output: Option<String>) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Reporter");
    println!("  ──────────────────────────────────────\n");

    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(&db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    let reporter = Reporter::new(db);

    let report_format = match format.to_lowercase().as_str() {
        "html" => ReportFormat::Html,
        "json" => ReportFormat::Json,
        "text" | "txt" => ReportFormat::Text,
        _ => {
            println!("  ⚠ Unknown format '{}', defaulting to text", format);
            ReportFormat::Text
        }
    };

    if let Some(output_path) = output {
        reporter
            .save_report(report_format, PathBuf::from(&output_path))
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        println!("  \u{1b}[38;2;0;212;170m✓ Report saved to {}\u{1b}[0m", output_path);
    } else {
        let content = match report_format {
            ReportFormat::Html => reporter.generate_html().await.map_err(|e| anyhow::anyhow!(e))?,
            ReportFormat::Text => reporter.generate_text().await.map_err(|e| anyhow::anyhow!(e))?,
            ReportFormat::Json => reporter.generate_json().await.map_err(|e| anyhow::anyhow!(e))?,
        };
        println!("{}", content);
    }

    Ok(())
}
