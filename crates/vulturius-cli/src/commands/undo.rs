use vulturius_core::config::AppConfig;
use vulturius_core::cleaner::Cleaner;
use vulturius_core::db::Database;

pub async fn handle(config: &AppConfig, count: usize) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Undo");
    println!("  ──────────────────────────────────────\n");

    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(&db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    let cleaner = Cleaner::new(config.clone(), db);
    let restored = cleaner.undo_last(count).await.map_err(|e| anyhow::anyhow!(e))?;

    println!("  \u{1b}[38;2;0;212;170m✓ Undone {} operation(s)\u{1b}[0m", restored);
    println!();
    Ok(())
}
