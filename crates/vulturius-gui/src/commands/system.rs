use serde::Serialize;
use vulturius_core::config::AppConfig;
use vulturius_core::db::Database;
use vulturius_core::platform::create_platform_ops;
use vulturius_core::types::AppAnalysis;

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub hostname: String,
    pub drives: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn get_drives() -> Result<serde_json::Value, String> {
    let platform = create_platform_ops();
    let drives = platform
        .list_drives()
        .map_err(|e| format!("Failed to list drives: {}", e))?;

    serde_json::to_value(drives)
        .map_err(|e| format!("Serialization error: {}", e))
}

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    let platform = create_platform_ops();
    let drives = platform
        .list_drives()
        .map_err(|e| format!("Failed to list drives: {}", e))?;

    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        hostname,
        drives: serde_json::to_value(drives)
            .map_err(|e| format!("Serialization error: {}", e))?
            .as_array()
            .cloned()
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn get_config() -> Result<serde_json::Value, String> {
    let config = AppConfig::default();
    serde_json::to_value(config).map_err(|e| format!("Serialization error: {}", e))
}

#[tauri::command]
pub async fn update_config(_settings: serde_json::Value) -> Result<(), String> {
    // In production, this would update the config file
    // For now, just acknowledge the update
    Ok(())
}

#[tauri::command]
pub async fn get_apps(db: tauri::State<'_, Database>) -> Result<Vec<AppAnalysis>, String> {
    db.get_all_apps()
        .await
        .map_err(|e| format!("Failed to get apps: {}", e))
}

#[tauri::command]
pub async fn get_quarantine(
    db: tauri::State<'_, Database>,
) -> Result<serde_json::Value, String> {
    let entries = db
        .get_quarantine_entries()
        .await
        .map_err(|e| format!("Failed to get quarantine: {}", e))?;

    serde_json::to_value(entries).map_err(|e| format!("Serialization error: {}", e))
}

#[tauri::command]
pub async fn restore_quarantine(
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    let entry = db
        .get_quarantine_entry(&id)
        .await
        .map_err(|e| format!("Failed to get quarantine entry: {}", e))?
        .ok_or("Quarantine entry not found".to_string())?;

    // Restore file from quarantine to original location
    std::fs::rename(
        &entry.quarantined_path,
        &entry.original_path,
    )
    .map_err(|e| format!("Failed to restore file: {}", e))?;

    // Remove from database
    db.remove_quarantine_entry(&id)
        .await
        .map_err(|e| format!("Failed to remove quarantine entry: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn purge_quarantine(
    db: tauri::State<'_, Database>,
) -> Result<usize, String> {
    let entries = db
        .get_quarantine_entries()
        .await
        .map_err(|e| format!("Failed to get quarantine entries: {}", e))?;

    let mut count = 0;
    for entry in &entries {
        if let Err(e) = std::fs::remove_file(&entry.quarantined_path) {
            tracing::warn!("Failed to delete quarantine file: {}", e);
        } else {
            count += 1;
        }
    }

    // Clear from database
    for entry in &entries {
        if let Err(e) = db.remove_quarantine_entry(&entry.id).await {
            tracing::warn!("Failed to remove quarantine entry: {}", e);
        }
    }

    Ok(count)
}
