use serde::Serialize;
use vulturius_core::analyzer::Analyzer;
use vulturius_core::config::AppConfig;
use vulturius_core::db::Database;
use vulturius_core::types::{FileAnalysis, RiskTolerance};

#[derive(serde::Deserialize)]
pub struct AnalyzeRequest {
    pub risk_tolerance: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AnalysisSummary {
    pub total_files: usize,
    pub safe_to_delete: usize,
    pub caution: usize,
    pub do_not_delete: usize,
    pub review_manually: usize,
    pub reclaimable_space: u64,
}

#[tauri::command]
pub async fn analyze_files(
    db: tauri::State<'_, Database>,
    request: AnalyzeRequest,
) -> Result<AnalysisSummary, String> {
    // Get latest scan
    let scan_id = db
        .get_latest_scan_id()
        .await
        .map_err(|e| format!("Failed to get scan: {}", e))?
        .ok_or("No scan results found. Run a scan first.".to_string())?;

    let scan = db
        .get_scan(&scan_id)
        .await
        .map_err(|e| format!("Failed to load scan: {}", e))?
        .ok_or("Scan not found".to_string())?;

    let risk_tolerance = match request.risk_tolerance.as_deref() {
        Some("paranoid") => RiskTolerance::Paranoid,
        Some("conservative") => RiskTolerance::Conservative,
        Some("aggressive") => RiskTolerance::Aggressive,
        Some("nuclear") => RiskTolerance::Nuclear,
        _ => RiskTolerance::Balanced,
    };

    let config = AppConfig::default();
    let analyzer = Analyzer::new(config);

    // analyze() is synchronous
    let analyses = analyzer.analyze(&scan);

    // Save analysis results
    db.save_analysis(&analyses)
        .await
        .map_err(|e| format!("Failed to save analysis: {}", e))?;

    // Build summary
    let mut summary = AnalysisSummary {
        total_files: analyses.len(),
        safe_to_delete: 0,
        caution: 0,
        do_not_delete: 0,
        review_manually: 0,
        reclaimable_space: 0,
    };

    for analysis in &analyses {
        match analysis.recommendation {
            vulturius_core::types::Recommendation::SafeToDelete => {
                summary.safe_to_delete += 1;
            }
            vulturius_core::types::Recommendation::Caution => {
                summary.caution += 1;
            }
            vulturius_core::types::Recommendation::DoNotDelete => {
                summary.do_not_delete += 1;
            }
            vulturius_core::types::Recommendation::ReviewManually => {
                summary.review_manually += 1;
            }
        }
    }

    Ok(summary)
}

#[tauri::command]
pub async fn get_analysis_results(
    db: tauri::State<'_, Database>,
    limit: Option<usize>,
) -> Result<Vec<FileAnalysis>, String> {
    // Get latest scan entries
    let scan_id = db
        .get_latest_scan_id()
        .await
        .map_err(|e| format!("Failed to get scan: {}", e))?
        .ok_or("No scan results found".to_string())?;

    let scan = db
        .get_scan(&scan_id)
        .await
        .map_err(|e| format!("Failed to load scan: {}", e))?
        .ok_or("Scan not found".to_string())?;

    let file_ids: Vec<String> = scan
        .entries
        .into_iter()
        .take(limit.unwrap_or(1000))
        .map(|e| e.id)
        .collect();

    db.get_analysis(&file_ids)
        .await
        .map_err(|e| format!("Failed to get analysis: {}", e))
}
