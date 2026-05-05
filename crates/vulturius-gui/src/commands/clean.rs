use serde::Serialize;
use vulturius_core::config::AppConfig;
use vulturius_core::db::Database;
use vulturius_core::types::{CleanupPlan, CleanupResult, CleanupItem, CleanupOperation, RiskTolerance};
use chrono::Utc;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
pub struct CleanupRequest {
    pub dry_run: Option<bool>,
    pub quarantine: Option<bool>,
    pub risk_tolerance: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GuiCleanupResult {
    pub items_processed: usize,
    pub items_succeeded: usize,
    pub items_failed: usize,
    pub space_freed: u64,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn execute_cleanup(
    db: tauri::State<'_, Database>,
    request: CleanupRequest,
) -> Result<GuiCleanupResult, String> {
    let risk_tolerance = match request.risk_tolerance.as_deref() {
        Some("paranoid") => RiskTolerance::Paranoid,
        Some("conservative") => RiskTolerance::Conservative,
        Some("aggressive") => RiskTolerance::Aggressive,
        Some("nuclear") => RiskTolerance::Nuclear,
        _ => RiskTolerance::Balanced,
    };

    let dry_run = request.dry_run.unwrap_or(false);
    let use_quarantine = request.quarantine.unwrap_or(true);

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

    // Build cleanup items from scan entries based on risk tolerance
    let min_score = risk_tolerance.min_score();
    let mut items = Vec::new();
    let mut estimated_space: u64 = 0;

    for entry in &scan.entries {
        // Use category safety score as confidence
        let confidence = entry.category.default_safety_score();
        if confidence >= min_score {
            let operation = if use_quarantine {
                CleanupOperation::Quarantine
            } else {
                CleanupOperation::Delete
            };
            estimated_space += entry.size;
            items.push(CleanupItem {
                file_id: entry.id.clone(),
                path: entry.path.clone(),
                size: entry.size,
                confidence_score: confidence,
                operation,
            });
        }
    }

    let plan = CleanupPlan {
        plan_id: uuid::Uuid::new_v4().to_string(),
        created_at: Utc::now(),
        risk_tolerance,
        items: items.clone(),
        estimated_freed_space: estimated_space,
    };

    // Execute cleanup
    let mut items_processed = 0;
    let mut items_succeeded = 0;
    let mut items_failed = 0;
    let mut space_freed: u64 = 0;
    let mut errors = Vec::new();

    for item in &plan.items {
        items_processed += 1;
        if dry_run {
            items_succeeded += 1;
            space_freed += item.size;
            continue;
        }

        match execute_single_item(&item, use_quarantine, &db).await {
            Ok(()) => {
                items_succeeded += 1;
                space_freed += item.size;
            }
            Err(e) => {
                items_failed += 1;
                errors.push(e);
            }
        }
    }

    // Save result to history
    let result = CleanupResult {
        plan_id: plan.plan_id.clone(),
        executed_at: Utc::now(),
        completed: items_failed == 0,
        items_processed,
        items_succeeded,
        items_failed,
        space_freed,
        errors: errors
            .iter()
            .map(|e| vulturius_core::types::CleanupError {
                path: String::new(),
                message: e.clone(),
            })
            .collect(),
    };

    db.save_cleanup(&result)
        .await
        .map_err(|e| format!("Failed to save cleanup result: {}", e))?;

    Ok(GuiCleanupResult {
        items_processed,
        items_succeeded,
        items_failed,
        space_freed,
        errors,
    })
}

async fn execute_single_item(
    item: &CleanupItem,
    use_quarantine: bool,
    db: &Database,
) -> Result<(), String> {
    let quarantine_base = std::path::PathBuf::from(".vulturius_data/quarantine");

    match item.operation {
        CleanupOperation::Delete => {
            std::fs::remove_file(&item.path)
                .map_err(|e| format!("Failed to delete {}: {}", item.path.display(), e))?;
        }
        CleanupOperation::Quarantine => {
            std::fs::create_dir_all(&quarantine_base)
                .map_err(|e| format!("Failed to create quarantine dir: {}", e))?;

            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let file_name = item
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("file_{}", timestamp));

            let dest = quarantine_base.join(format!("{}_{}", timestamp, file_name));

            std::fs::rename(&item.path, &dest)
                .map_err(|e| format!("Failed to quarantine {}: {}", item.path.display(), e))?;

            // Log to database
            let entry = vulturius_core::types::QuarantineEntry {
                id: uuid::Uuid::new_v4().to_string(),
                original_path: item.path.clone(),
                quarantined_path: dest,
                size: item.size,
                quarantined_at: Utc::now(),
                reason: "Cleanup operation".to_string(),
            };
            db.save_quarantine_entry(&entry)
                .await
                .map_err(|e| format!("Failed to log quarantine: {}", e))?;
        }
        _ => {
            // Other operations are handled differently
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_cleanup_status() -> Result<String, String> {
    Ok("No cleanup in progress".to_string())
}

#[tauri::command]
pub async fn get_history(
    db: tauri::State<'_, Database>,
    limit: Option<usize>,
) -> Result<serde_json::Value, String> {
    let history = db
        .get_cleanup_history(limit.unwrap_or(20))
        .await
        .map_err(|e| format!("Failed to get history: {}", e))?;

    serde_json::to_value(history).map_err(|e| format!("Serialization error: {}", e))
}
