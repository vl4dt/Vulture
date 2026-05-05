use vulturius_core::config::AppConfig;
use vulturius_core::analyzer::Analyzer;
use vulturius_core::types::RiskTolerance;
use vulturius_core::db::Database;
use std::path::PathBuf;

pub async fn handle(config: &AppConfig) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Analyzer");
    println!("  ──────────────────────────────────────\n");

    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(db_path).await.map_err(|e| anyhow::anyhow!(e))?;

    // Get latest scan
    let latest_id = db.get_latest_scan_id().await.map_err(|e| anyhow::anyhow!(e))?;
    match latest_id {
        Some(id) => {
            println!("  Loading scan: {}", id);
            let scan = db.get_scan(&id).await.map_err(|e| anyhow::anyhow!(e))?;
            match scan {
                Some(scan_result) => {
                    let analyzer = Analyzer::new(config.clone());
                    let analyses = analyzer.analyze(&scan_result);

                    // Save to DB
                    db.save_analysis(&analyses).await.map_err(|e| anyhow::anyhow!(e))?;

                    let risk_tol = RiskTolerance::Balanced;
                    let safe = analyses.iter().filter(|a| a.confidence_score >= risk_tol.min_score()).count();
                    let total = analyses.len();

                    println!("\n  \u{1b}[38;2;0;212;170m✓ Re-analysis complete\u{1b}[0m");
                    println!("    Files analyzed : {}", total);
                    println!("    Safe to delete : \u{1b}[38;2;0;212;170m{}\u{1b}[0m", safe);
                    println!("    Skip           : \u{1b}[38;2;233;233;255m{}\u{1b}[0m", total - safe);
                }
                None => println!("  ⚠ No scan data found for ID: {}", id),
            }
        }
        None => {
            println!("  ⚠ No previous scan found. Run `vulturius scan` first.");
        }
    }

    // Also show app status
    let apps = db.get_all_apps().await.map_err(|e| anyhow::anyhow!(e))?;
    if !apps.is_empty() {
        println!("\n  \u{1b}[38;2;0;240;255m⠿ Application Registry\u{1b}[0m");
        let unused = apps.iter().filter(|a| matches!(a.recommendation, vulturius_core::types::AppRecommendation::Unused)).count();
        println!("    Total apps     : {}", apps.len());
        println!("    Unused         : \u{1b}[38;2;255;45;85m{}\u{1b}[0m", unused);
    }

    println!();
    Ok(())
}
