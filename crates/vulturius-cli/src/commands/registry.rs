use vulturius_core::config::AppConfig;
use vulturius_core::registry::{RegistryScanner, RegistryCleaner, RegistryEntry};
use vulturius_core::db::Database;

/// Registry subcommand passed from main.
#[derive(Debug, Clone)]
pub enum RegistrySubCommand {
    /// Scan for orphaned/unused entries
    Scan,
    /// Clean orphaned entries
    Clean { dry_run: bool },
}

pub async fn handle(config: &AppConfig, sub_cmd: RegistrySubCommand) -> anyhow::Result<()> {
    // Initialize database
    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    std::fs::create_dir_all(&db_dir).ok();
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    // Get known apps from database for cross-referencing
    let known_apps = db.get_all_apps().await.unwrap_or_default();

    match sub_cmd {
        RegistrySubCommand::Scan => {
            handle_scan(&config, &known_apps).await
        }
        RegistrySubCommand::Clean { dry_run } => {
            handle_clean(&config, &known_apps, dry_run).await
        }
    }
}

async fn handle_scan(_config: &AppConfig, known_apps: &[vulturius_core::types::AppAnalysis]) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Registry Scanner");
    println!("  ──────────────────────────────────────\n");
    println!("  Scanning Windows Registry for orphaned entries...\n");

    let scanner = RegistryScanner::new().with_known_apps(known_apps);
    let result = scanner.full_scan().await?;

    println!("  ── Scan Results ─────────────────────────────");
    println!("  Total keys scanned:    {}", result.total_keys_scanned);
    println!("  Orphaned entries:      {}", result.orphaned_entries.len());
    println!("  Unused keys:           {}", result.unused_keys.len());
    println!("  Invalid uninstall rec: {}", result.invalid_uninstall_records.len());
    println!("  Estimated cleanable:   {}\n", result.estimated_cleanable);

    if !result.orphaned_entries.is_empty() {
        println!("  ── Orphaned Entries ────────────────────────");
        for (i, entry) in result.orphaned_entries.iter().enumerate() {
            println!("  {}. [ORPHAN] {}", i + 1, entry.value_data);
            println!("     Key:     {}", entry.key_path);
            println!("     Reason:  {}", entry.reason);
        }
        println!();
    }

    if !result.unused_keys.is_empty() {
        println!("  ── Unused Keys ────────────────────────────");
        for (i, entry) in result.unused_keys.iter().enumerate() {
            println!("  {}. [UNUSED] {}", i + 1, entry.value_name);
            println!("     Key:     {}", entry.key_path);
            println!("     Reason:  {}", entry.reason);
        }
        println!();
    }

    if !result.invalid_uninstall_records.is_empty() {
        println!("  ── Invalid Uninstall Records ──────────────");
        for (i, record) in result.invalid_uninstall_records.iter().enumerate() {
            println!("  {}. {}", i + 1, record.display_name);
            println!("     Install exists: {}", record.install_exists);
            println!("     Uninstall valid:  {}", record.uninstall_valid);
        }
        println!();
    }

    if result.estimated_cleanable == 0 {
        println!("  \u{1b}[38;2;0;212;170m✓\u{1b}[0m No orphaned registry entries found.");
    } else {
        println!("  Run `vulturius registry clean` to remove orphaned entries.");
    }

    Ok(())
}

async fn handle_clean(
    _config: &AppConfig,
    known_apps: &[vulturius_core::types::AppAnalysis],
    dry_run: bool,
) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Registry Cleaner");
    println!("  ──────────────────────────────────────\n");

    if dry_run {
        println!("  \u{1b}[38;2;255;184;0m⚠ DRY RUN MODE\u{1b}[0m — No changes will be made\n");
    }

    println!("  Scanning for orphaned entries...\n");

    let scanner = RegistryScanner::new().with_known_apps(known_apps);
    let result = scanner.full_scan().await?;

    let all_entries: Vec<RegistryEntry> = result
        .orphaned_entries
        .iter()
        .chain(result.unused_keys.iter())
        .cloned()
        .collect();

    if all_entries.is_empty() {
        println!("  \u{1b}[38;2;0;212;170m✓\u{1b}[0m No orphaned entries to clean.");
        return Ok(());
    }

    println!("  Found {} entries to clean:", all_entries.len());

    for entry in &all_entries {
        let tag = if entry.key_path.contains("Uninstall") {
            "\u{1b}[38;2;255;45;85m[ORPHAN]\u{1b}[0m"
        } else {
            "\u{1b}[38;2;255;184;0m[UNUSED]\u{1b}[0m"
        };
        println!("  {} {} — {}", tag, entry.value_data, entry.reason);
    }

    println!();

    let cleaner = RegistryCleaner::new();
    let cleaned = cleaner.clean_orphaned_entries(&all_entries, dry_run).await?;

    println!();
    if dry_run {
        println!("  \u{1b}[38;2;0;212;170m✓\u{1b}[0m Dry run complete. {} entries would be cleaned.", cleaned);
    } else {
        println!("  \u{1b}[38;2;0;212;170m✓\u{1b}[0m Cleaned {} registry entries.", cleaned);
    }

    Ok(())
}
