#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            use tauri::Manager;
            use std::sync::Arc;

            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();

            let db_path = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".vulturius_data")
                .join("vulturius.db");

            // Ensure parent directory exists
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let db_path_str = db_path.to_string_lossy().to_string();
            tracing::info!("DB path: {}", db_path_str);

            // Create database and initialize in one async block
            let db = tauri::async_runtime::block_on(async move {
                match vulturius_core::Database::new(&db_path_str).await {
                    Ok(db) => {
                        let _ = db.init().await;
                        Ok(Arc::new(db))
                    }
                    Err(e) => {
                        tracing::warn!("DB init failed, running without DB: {}", e);
                        Err(e)
                    }
                }
            });

            // Always manage ScanState
            app.manage(commands::scan::ScanState::new());

            // Manage DB if available
            if let Ok(db) = db {
                app.manage(db);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan::start_scan,
            commands::scan::get_scan_status,
            commands::scan::cancel_scan,
            commands::analyze::analyze_files,
            commands::analyze::get_analysis_results,
            commands::clean::execute_cleanup,
            commands::clean::get_cleanup_status,
            commands::clean::get_history,
            commands::system::get_drives,
            commands::system::get_system_info,
            commands::system::get_config,
            commands::system::update_config,
            commands::system::get_apps,
            commands::system::get_quarantine,
            commands::system::restore_quarantine,
            commands::system::purge_quarantine,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run Vulturius GUI");
}
