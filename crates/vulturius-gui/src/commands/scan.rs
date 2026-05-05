use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use vulturius_core::config::AppConfig;
use vulturius_core::db::Database;
use vulturius_core::scanner::Scanner;
use vulturius_core::types::{ScanTarget, ScanType};

/// Shared scan state
pub struct ScanState {
    pub in_progress: Mutex<bool>,
}

impl ScanState {
    pub fn new() -> Self {
        Self {
            in_progress: Mutex::new(false),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    pub target_path: Option<String>,
    pub scan_type: Option<String>,
    pub quick: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ScanStatus {
    pub in_progress: bool,
    pub files_scanned: usize,
    pub dirs_scanned: usize,
    pub total_size: u64,
    pub percentage: f64,
    pub message: String,
}

#[tauri::command]
pub async fn start_scan(
    scan_state: tauri::State<'_, ScanState>,
    db: tauri::State<'_, Database>,
    request: ScanRequest,
) -> Result<String, String> {
    if *scan_state.in_progress.lock().unwrap() {
        return Err("Scan already in progress".to_string());
    }

    *scan_state.in_progress.lock().unwrap() = true;

    let config = AppConfig::default();
    let scan_type = match request.scan_type.as_deref() {
        Some("quick") => ScanType::Quick,
        Some("full") => ScanType::Full,
        _ => ScanType::Full,
    };

    let target_path = request.target_path.unwrap_or_else(|| String::from("/"));

    let target = ScanTarget {
        path: std::path::PathBuf::from(target_path),
        scan_type: scan_type.clone(),
        exclusions: config.scan.default_exclusions.clone(),
        max_depth: if request.quick.unwrap_or(false) { Some(5) } else { None },
        file_types: None,
        collect_hashes: false,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let scanner = Scanner::new(config);

    let scan_id = uuid::Uuid::new_v4().to_string();

    let scan_handle = tokio::spawn(async move {
        scanner.scan(vec![target], Some(tx)).await
    });

    let progress_handle = tokio::spawn(async move {
        while rx.recv().await.is_some() {}
    });

    let result = tokio::time::timeout(std::time::Duration::from_secs(300), scan_handle).await;

    let scan_result = match result {
        Ok(Ok(Ok(sr))) => sr,
        Ok(Ok(Err(e))) => {
            *scan_state.in_progress.lock().unwrap() = false;
            progress_handle.abort();
            return Err(format!("Scan failed: {}", e));
        }
        Ok(Err(_)) => {
            *scan_state.in_progress.lock().unwrap() = false;
            progress_handle.abort();
            return Err("Scan task panicked".to_string());
        }
        Err(_) => {
            *scan_state.in_progress.lock().unwrap() = false;
            progress_handle.abort();
            return Err("Scan timed out".to_string());
        }
    };

    if let Err(e) = db.save_scan(&scan_result).await {
        tracing::error!("Failed to save scan result: {}", e);
    }

    *scan_state.in_progress.lock().unwrap() = false;
    progress_handle.abort();

    Ok(scan_id)
}

#[tauri::command]
pub async fn get_scan_status(scan_state: tauri::State<'_, ScanState>) -> Result<ScanStatus, String> {
    let in_progress = *scan_state.in_progress.lock().unwrap();
    Ok(ScanStatus {
        in_progress,
        files_scanned: 0,
        dirs_scanned: 0,
        total_size: 0,
        percentage: 0.0,
        message: if in_progress {
            "Scanning...".to_string()
        } else {
            "Ready".to_string()
        },
    })
}

#[tauri::command]
pub async fn cancel_scan(scan_state: tauri::State<'_, ScanState>) -> Result<(), String> {
    let mut in_progress = scan_state.in_progress.lock().unwrap();
    if *in_progress {
        *in_progress = false;
        Ok(())
    } else {
        Err("No scan in progress".to_string())
    }
}
