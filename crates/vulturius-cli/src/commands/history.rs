use vulturius_core::config::AppConfig;
use vulturius_core::db::Database;

pub async fn handle(_config: &AppConfig) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Cleanup History");
    println!("  ──────────────────────────────────────\n");

    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    let history = db.get_cleanup_history(10).await.map_err(|e| anyhow::anyhow!(e))?;

    if history.is_empty() {
        println!("  No cleanup history found.");
    } else {
        for entry in &history {
            let status = if entry.completed {
                "\u{1b}[38;2;0;212;170m✓\u{1b}[0m"
            } else {
                "\u{1b}[38;2;255;45;85m⚠\u{1b}[0m"
            };
            println!(
                "  {} {} — {} items, freed {}",
                status,
                entry.executed_at.format("%Y-%m-%d %H:%M"),
                entry.items_processed,
                humansize::format_size(entry.space_freed, humansize::DECIMAL),
            );
        }
    }

    println!();
    Ok(())
}
