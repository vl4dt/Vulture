use vulturius_core::config::AppConfig;
use vulturius_core::scanner::Scanner;
use vulturius_core::analyzer::{Analyzer, AppDetector};
use vulturius_core::cleaner::Cleaner;
use vulturius_core::types::{RiskTolerance, ScanType, Recommendation};
use vulturius_core::db::Database;
use vulturius_core::ScanTarget;
use std::path::PathBuf;
use std::time::Instant;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn handle(
    config: &AppConfig,
    dry_run: bool,
    quarantine: bool,
    risk_tolerance: String,
    quick: bool,
    confirm: bool,
) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Cleaner");
    println!("  ──────────────────────────────────────\n");

    let risk_tol = parse_risk_tolerance(&risk_tolerance);

    // Initialize DB
    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    std::fs::create_dir_all(&db_dir).ok();
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(&db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    // Scan
    let targets = build_targets(config, quick);
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let scanner = Scanner::new(config.clone());

    let scan_handle = tokio::spawn(async move {
        scanner.scan(targets, Some(tx)).await
    });

    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message("Scanning filesystem...");

    let start = Instant::now();
    while let Some(progress) = rx.recv().await {
        match progress {
            vulturius_core::scanner::ScanProgress::Progress { files_scanned, bytes_scanned, .. } => {
                pb.set_message(format!(
                    "{} files | {}",
                    files_scanned,
                    humansize::format_size(bytes_scanned, humansize::DECIMAL),
                ));
            }
            vulturius_core::scanner::ScanProgress::Complete { total_files, total_size, duration, .. } => {
                pb.finish_and_clear();
                println!("\n  \u{1b}[38;2;0;212;170m✓ Scan complete\u{1b}[0m");
                println!("    Files scanned : {}", total_files);
                println!("    Data scanned  : {}", humansize::format_size(total_size, humansize::DECIMAL));
                println!("    Duration      : {:.2}s", duration.as_secs_f32());
            }
            _ => {}
        }
    }

    let scan_result = scan_handle.await.map_err(|e| anyhow::anyhow!(e))??;

    // Save to DB
    db.save_scan(&scan_result).await.map_err(|e| anyhow::anyhow!(e))?;

    // Analyze
    println!("\n  \u{1b}[38;2;0;240;255m⠿ Analyzing...\u{1b}[0m");
    let analyzer = Analyzer::new(config.clone());
    let analyses = analyzer.analyze(&scan_result);
    db.save_analysis(&analyses).await.map_err(|e| anyhow::anyhow!(e))?;

    // Build cleanup plan (move db into cleaner)
    let cleaner = Cleaner::new(config.clone(), db);
    let plan = cleaner.build_plan(&scan_result, &analyses, risk_tol.clone(), quarantine);

    let safe_count = plan.items.len();
    let estimated_space = plan.estimated_freed_space;

    println!("\n  \u{1b}[38;2;0;212;170m✓ Analysis complete\u{1b}[0m");
    println!("    Items to clean : {}", safe_count);
    println!("    Est. space     : {}", humansize::format_size(estimated_space, humansize::DECIMAL));
    println!("    Risk tolerance : {}", risk_tol);
    println!("    Mode           : {}", if dry_run { "DRY RUN" } else { if quarantine { "QUARANTINE" } else { "DELETE" }});

    // Confirmation
    if !dry_run && confirm && safe_count > 0 {
        println!("\n  ⚠ This will {} {} items (~{})", 
            if quarantine { "quarantine" } else { "delete" },
            safe_count,
            humansize::format_size(estimated_space, humansize::DECIMAL),
        );
        print!("  Continue? [y/N]: ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  Cancelled.");
            return Ok(());
        }
    }

    // Execute cleanup
    println!("\n  \u{1b}[38;2;0;240;255m⠿ Cleaning...\u{1b}[0m");
    let result = cleaner.execute_plan(&plan, dry_run).await.map_err(|e| anyhow::anyhow!(e))?;

    println!("\n  \u{1b}[38;2;0;212;170m✓ Cleanup complete\u{1b}[0m");
    println!("    Processed    : {}", result.items_processed);
    println!("    Succeeded    : \u{1b}[38;2;0;212;170m{}\u{1b}[0m", result.items_succeeded);
    println!("    Failed       : \u{1b}[38;2;255;45;85m{}\u{1b}[0m", result.items_failed);
    println!("    Space freed  : {}", humansize::format_size(result.space_freed, humansize::DECIMAL));
    println!("    Total time   : {:.2}s", start.elapsed().as_secs_f32());

    if !result.errors.is_empty() {
        println!("\n  Errors:");
        for err in &result.errors {
            println!("    ⚠ {} — {}", err.path, err.message);
        }
    }

    println!();
    Ok(())
}

fn build_targets(config: &AppConfig, quick: bool) -> Vec<ScanTarget> {
    let mut targets = Vec::new();

    #[cfg(target_os = "windows")]
    {
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            if PathBuf::from(&drive).exists() {
                targets.push(ScanTarget {
                    path: PathBuf::from(drive),
                    scan_type: if quick { ScanType::Quick } else { ScanType::Full },
                    exclusions: config.scan.default_exclusions.clone(),
                    max_depth: if quick { Some(5) } else { None },
                    file_types: None,
                    collect_hashes: config.scan.collect_hashes,
                });
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            targets.push(ScanTarget {
                path: PathBuf::from(home),
                scan_type: if quick { ScanType::Quick } else { ScanType::Full },
                exclusions: config.scan.default_exclusions.clone(),
                max_depth: if quick { Some(5) } else { None },
                file_types: None,
                collect_hashes: config.scan.collect_hashes,
            });
        }
    }

    if targets.is_empty() {
        targets.push(ScanTarget {
            path: PathBuf::from("/"),
            scan_type: if quick { ScanType::Quick } else { ScanType::Full },
            exclusions: config.scan.default_exclusions.clone(),
            max_depth: if quick { Some(5) } else { None },
            file_types: None,
            collect_hashes: config.scan.collect_hashes,
        });
    }

    targets
}

fn parse_risk_tolerance(s: &str) -> RiskTolerance {
    match s.to_lowercase().as_str() {
        "paranoid" => RiskTolerance::Paranoid,
        "conservative" => RiskTolerance::Conservative,
        "aggressive" => RiskTolerance::Aggressive,
        "nuclear" => RiskTolerance::Nuclear,
        _ => RiskTolerance::Balanced,
    }
}
