use vulturius_core::config::AppConfig;
use vulturius_core::scanner::{Scanner, ScanProgress};
use vulturius_core::{ScanTarget, ScanType};
use vulturius_core::analyzer::{Analyzer, AppDetector};
use vulturius_core::types::RiskTolerance;
use vulturius_core::db::Database;
use std::path::PathBuf;
use std::time::Instant;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn handle(
    config: &AppConfig,
    quick: bool,
    depth: Option<usize>,
    analyze: bool,
    risk_tolerance: String,
) -> anyhow::Result<()> {
    println!("\n  \u{1b}[38;2;0;240;255m⠿ VULTURIUS\u{1b}[0m — Disk Scanner");
    println!("  ──────────────────────────────────────\n");

    let risk_tol = parse_risk_tolerance(&risk_tolerance);

    // Build scan targets
    let targets = build_targets(config, quick, depth);

    // Create progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} {msg}\n  {bar:40.cyan/blue} {percent}%")
            .unwrap()
            .progress_chars("█▉▊█▇▆▅▄▃▂▁  "),
    );
    pb.set_message("Initializing scan...");

    // Initialize database in project data directory
    let db_dir = std::env::current_dir().unwrap().join(".vulturius_data");
    std::fs::create_dir_all(&db_dir).ok();
    let db_path = db_dir.join("vulturius.db").to_string_lossy().to_string();
    let db = Database::new(db_path).await.map_err(|e| anyhow::anyhow!(e))?;
    db.init().await.map_err(|e| anyhow::anyhow!(e))?;

    // Run scan
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let scanner = Scanner::new(config.clone());
    let config_clone = config.clone();

    let scan_handle = tokio::spawn(async move {
        scanner.scan(targets, Some(tx)).await
    });

    // Consume progress and update bar
    let start = Instant::now();
    while let Some(progress) = rx.recv().await {
        match progress {
            ScanProgress::Started { total_targets } => {
                pb.set_message(format!("Scanning {} target(s)...", total_targets));
            }
            ScanProgress::Progress { files_scanned, bytes_scanned, .. } => {
                let elapsed = start.elapsed().as_secs_f32();
                let rate = if elapsed > 0.0 { files_scanned as f32 / elapsed } else { 0.0 };
                pb.set_message(format!(
                    "{} files | {} | {:.0} files/s",
                    files_scanned,
                    humansize::format_size(bytes_scanned as u64, humansize::DECIMAL),
                    rate,
                ));
                pb.set_position((files_scanned % 10000) as u64);
            }
            ScanProgress::Error { .. } => {
                // Silently counted in final results
            }
            ScanProgress::Complete { total_files, total_size, duration, .. } => {
                pb.finish_and_clear();
                println!("\n  \u{1b}[38;2;0;212;170m✓ Scan complete\u{1b}[0m");
                println!("    Files scanned : {}", total_files);
                println!("    Data scanned  : {}", humansize::format_size(total_size, humansize::DECIMAL));
                println!("    Duration      : {:.2}s", duration.as_secs_f32());
            }
            ScanProgress::Cancelled => {
                pb.finish_and_clear();
                println!("\n  ⚠ Scan cancelled");
                return Ok(());
            }
        }
    }

    let scan_result = scan_handle.await.map_err(|e| anyhow::anyhow!(e))??;

    // Save to database
    db.save_scan(&scan_result).await.map_err(|e| anyhow::anyhow!(e))?;
    println!("    Saved to DB   : ✓");

    // Analyze if requested
    if analyze {
        println!("\n  \u{1b}[38;2;0;240;255m⠿ Analyzing...\u{1b}[0m");

        let analyzer = Analyzer::new(config_clone.clone());
        let analyses = analyzer.analyze(&scan_result);

        // Save analysis to DB
        db.save_analysis(&analyses).await.map_err(|e| anyhow::anyhow!(e))?;

        let safe_count = analyses
            .iter()
            .filter(|a| a.confidence_score >= risk_tol.min_score())
            .count();
        let caution_count = analyses.iter().filter(|a| matches!(a.recommendation, vulturius_core::types::Recommendation::Caution)).count();
        let skip_count = analyses.len() - safe_count - caution_count;

        println!("\n  \u{1b}[38;2;0;212;170m✓ Analysis complete\u{1b}[0m");
        println!("    Risk tolerance: {}", risk_tol);
        println!("    Safe to delete : \u{1b}[38;2;0;212;170m{}\u{1b}[0m", safe_count);
        println!("    Caution        : \u{1b}[38;2;255;184;0m{}\u{1b}[0m", caution_count);
        println!("    Skip           : \u{1b}[38;2;233;233;255m{}\u{1b}[0m", skip_count);

        // Detect apps
        println!("\n  \u{1b}[38;2;0;240;255m⠿ Detecting applications...\u{1b}[0m");
        let detector = AppDetector::new(config_clone.clone());
        let apps = detector.detect_apps().map_err(|e| anyhow::anyhow!(e))?;

        // Save apps to DB
        for app in &apps {
            db.save_app(app).await.map_err(|e| anyhow::anyhow!(e))?;
        }

        let unused = apps.iter().filter(|a| matches!(a.recommendation, vulturius_core::types::AppRecommendation::Unused)).count();
        let rarely = apps.iter().filter(|a| matches!(a.recommendation, vulturius_core::types::AppRecommendation::RarelyUsed)).count();
        let active = apps.iter().filter(|a| matches!(a.recommendation, vulturius_core::types::AppRecommendation::Active)).count();

        println!("\n  \u{1b}[38;2;0;212;170m✓ App detection complete\u{1b}[0m");
        println!("    Total apps     : {}", apps.len());
        println!("    Unused         : \u{1b}[38;2;255;45;85m{}\u{1b}[0m", unused);
        println!("    Rarely used    : \u{1b}[38;2;255;184;0m{}\u{1b}[0m", rarely);
        println!("    Active         : \u{1b}[38;2;0;212;170m{}\u{1b}[0m", active);
        println!();
    }

    Ok(())
}

fn build_targets(config: &AppConfig, quick: bool, depth: Option<usize>) -> Vec<ScanTarget> {
    // On Windows, scan all fixed drives. On Unix, scan home + /.
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
                    max_depth: if quick { Some(5) } else { depth },
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
                max_depth: if quick { Some(5) } else { depth },
                file_types: None,
                collect_hashes: config.scan.collect_hashes,
            });
        }
    }

    // Fallback if no targets found
    if targets.is_empty() {
        targets.push(ScanTarget {
            path: PathBuf::from("/"),
            scan_type: if quick { ScanType::Quick } else { ScanType::Full },
            exclusions: config.scan.default_exclusions.clone(),
            max_depth: if quick { Some(5) } else { depth },
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
