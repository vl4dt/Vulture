use clap::Subcommand;
use vulturius_core::config::AppConfig;
use vulturius_core::cleaner::Cleaner;
use vulturius_core::db::Database;

#[derive(Subcommand)]
pub enum QuarantineCommand {
    /// List quarantined files
    List,
    /// Restore a quarantined file
    Restore { id: String },
    /// Purge all quarantined files
    Purge,
}

pub async fn handle(config: &AppConfig, subcmd: QuarantineCommand) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Quarantine");
    println!("  ──────────────────────────────────────\n");

    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(&db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    let cleaner = Cleaner::new(config.clone(), db);

    match subcmd {
        QuarantineCommand::List => {
            let entries = cleaner.list_quarantine().await.map_err(|e| anyhow::anyhow!(e))?;
            if entries.is_empty() {
                println!("  No quarantined files.");
            } else {
                println!("  {} quarantined file(s):\n", entries.len());
                for entry in &entries {
                    println!(
                        "  \u{1b}[38;2;0;240;255m{}\u{1b}[0m  {}  ({})",
                        entry.id.chars().take(8).collect::<String>(),
                        entry.original_path.display(),
                        humansize::format_size(entry.size, humansize::DECIMAL),
                    );
                }
            }
        }
        QuarantineCommand::Restore { id } => {
            println!("  Restoring file {}...", id);
            cleaner
                .restore_from_quarantine(&id)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("  \u{1b}[38;2;0;212;170m✓ Restored\u{1b}[0m");
        }
        QuarantineCommand::Purge => {
            let entries = cleaner.list_quarantine().await.map_err(|e| anyhow::anyhow!(e))?;
            if entries.is_empty() {
                println!("  Nothing to purge.");
                return Ok(());
            }
            println!("  ⚠ This will permanently delete {} quarantined file(s).", entries.len());
            print!("  Continue? [y/N]: ");
            std::io::Write::flush(&mut std::io::stdout()).ok();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            if input.trim().eq_ignore_ascii_case("y") {
                let freed = cleaner.purge_quarantine().await.map_err(|e| anyhow::anyhow!(e))?;
                println!("  \u{1b}[38;2;0;212;170m✓ Purged, freed {}\u{1b}[0m", humansize::format_size(freed, humansize::DECIMAL));
            } else {
                println!("  Cancelled.");
            }
        }
    }

    println!();
    Ok(())
}
