use vulturius_core::config::AppConfig;
use vulturius_core::analyzer::AppDetector;
use vulturius_core::db::Database;
use std::path::PathBuf;

pub async fn handle(config: &AppConfig) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — App Detector");
    println!("  ──────────────────────────────────────\n");

    let detector = AppDetector::new(config.clone());
    let apps = detector.detect_apps().map_err(|e| anyhow::anyhow!(e))?;

    // Save to DB
    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    std::fs::create_dir_all(&db_dir).ok();
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    for app in &apps {
        db.save_app(app).await.map_err(|e| anyhow::anyhow!(e))?;
    }

    println!("  Detected {} applications:\n", apps.len());

    for app in &apps {
        let indicator = match app.recommendation {
            vulturius_core::types::AppRecommendation::Essential => "\u{1b}[38;2;0;212;170m●\u{1b}[0m",
            vulturius_core::types::AppRecommendation::Active => "\u{1b}[38;2;0;240;255m●\u{1b}[0m",
            vulturius_core::types::AppRecommendation::RarelyUsed => "\u{1b}[38;2;255;184;0m●\u{1b}[0m",
            vulturius_core::types::AppRecommendation::Unused => "\u{1b}[38;2;255;45;85m●\u{1b}[0m",
        };
        println!(
            "  {} {} ({})",
            indicator,
            app.name,
            humansize::format_size(app.total_size, humansize::DECIMAL),
        );
    }

    println!();
    Ok(())
}
