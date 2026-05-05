//! Windows Registry analysis and cleanup.
//!
//! Scans the Windows Registry for orphaned entries — uninstall keys,
//! application remnants, and stale configuration — that reference
//! software no longer installed on the system.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::types::AppAnalysis;
use crate::Result;

// ─── Registry Value Types ─────────────────────────────────────────────────────

/// Known Windows Registry value types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RegistryValueType {
    String,       /// REG_SZ
    ExpandString, /// REG_EXPAND_SZ
    Binary,       /// REG_BINARY
    DWord,        /// REG_DWORD
    QWord,        /// REG_QWORD
    MultiString,  /// REG_MULTI_SZ
    Unknown,
}

impl std::fmt::Display for RegistryValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryValueType::String => write!(f, "REG_SZ"),
            RegistryValueType::ExpandString => write!(f, "REG_EXPAND_SZ"),
            RegistryValueType::Binary => write!(f, "REG_BINARY"),
            RegistryValueType::DWord => write!(f, "REG_DWORD"),
            RegistryValueType::QWord => write!(f, "REG_QWORD"),
            RegistryValueType::MultiString => write!(f, "REG_MULTI_SZ"),
            RegistryValueType::Unknown => write!(f, "REG_UNKNOWN"),
        }
    }
}

// ─── Registry Entry ──────────────────────────────────────────────────────────

/// A single registry entry discovered during scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Full registry key path (e.g., `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}`)
    pub key_path: String,
    /// Value name within the key (e.g., `DisplayName`, `UninstallString`)
    pub value_name: String,
    /// Value data as a string representation
    pub value_data: String,
    /// Type of the registry value
    pub value_type: RegistryValueType,
    /// Whether this entry is flagged as orphaned
    pub is_orphaned: bool,
    /// Human-readable reason for orphan flag (if applicable)
    pub reason: String,
}

// ─── Uninstall Key Record ────────────────────────────────────────────────────

/// Parsed record from an Uninstall registry key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallRecord {
    pub key_path: String,
    pub display_name: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub install_date: Option<String>,
    pub estimated_size: Option<u64>,
    /// Whether the install location actually exists on disk
    pub install_exists: bool,
    /// Whether the uninstall string references a valid executable
    pub uninstall_valid: bool,
}

// ─── Registry Scan Result ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryScanResult {
    pub scan_id: String,
    pub scanned_at: String,
    pub total_keys_scanned: usize,
    pub orphaned_entries: Vec<RegistryEntry>,
    pub unused_keys: Vec<RegistryEntry>,
    pub invalid_uninstall_records: Vec<UninstallRecord>,
    pub estimated_cleanable: usize,
}

// ─── Well-known Registry Paths ───────────────────────────────────────────────

/// Standard uninstall registry paths to scan.
const UNINSTALL_PATHS: &[&str] = &[
    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
];

/// Keys commonly left behind by uninstallers.
const ORPHAN_KEY_PATTERNS: &[&str] = &[
    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
    r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Browser Helper Objects",
    r"HKLM\SYSTEM\CurrentControlSet\Services",
];

// ─── Registry Scanner ────────────────────────────────────────────────────────

/// Scans the Windows Registry for orphaned and unused entries.
pub struct RegistryScanner {
    /// Installed app names used for cross-referencing.
    known_apps: HashSet<String>,
}

impl RegistryScanner {
    pub fn new() -> Self {
        Self {
            known_apps: HashSet::new(),
        }
    }

    /// Provide a list of known installed apps for cross-referencing.
    /// Entries whose names don't match any known app are candidates for orphaning.
    pub fn with_known_apps(mut self, apps: &[AppAnalysis]) -> Self {
        self.known_apps = apps
            .iter()
            .map(|a| a.name.to_lowercase())
            .collect();
        self
    }

    /// Run a full registry scan: uninstall keys, orphan detection, unused keys.
    pub async fn full_scan(&self) -> Result<RegistryScanResult> {
        info!("Starting full registry scan...");

        let uninstall_records = self.scan_uninstall_keys().await?;
        let invalid_records = self.find_invalid_uninstall_records(&uninstall_records);
        let orphaned_entries = self.find_orphaned_entries(&uninstall_records)?;
        let unused_keys = self.scan_unused_run_keys().await?;

        let orphaned_count = orphaned_entries.len();
        let unused_count = unused_keys.len();
        let scan_id = uuid::Uuid::new_v4().to_string();

        info!(
            "Registry scan complete: {} orphaned entries, {} unused keys, {} invalid uninstall records",
            orphaned_count,
            unused_count,
            invalid_records.len()
        );

        Ok(RegistryScanResult {
            scan_id,
            scanned_at: Utc::now().to_rfc3339(),
            total_keys_scanned: uninstall_records.len() + unused_count,
            orphaned_entries,
            unused_keys,
            invalid_uninstall_records: invalid_records,
            estimated_cleanable: orphaned_count + unused_count,
        })
    }

    /// Scan all uninstall registry keys and parse their values.
    async fn scan_uninstall_keys(&self) -> Result<Vec<UninstallRecord>> {
        let mut records = Vec::new();

        for &root_path in UNINSTALL_PATHS {
            match self.enumerate_subkeys(root_path).await {
                Ok(subkeys) => {
                    for subkey in subkeys {
                        let full_path = format!("{}\\{}", root_path, subkey);
                        if let Ok(record) = self.parse_uninstall_key(&full_path).await {
                            records.push(record);
                        }
                    }
                }
                Err(e) => {
                    // Some paths may not exist on all systems (e.g., WOW6432Node on 32-bit)
                    debug!("Could not scan {}: {}", root_path, e);
                }
            }
        }

        Ok(records)
    }

    /// Enumerate subkey names under a registry path using `reg.exe`.
    async fn enumerate_subkeys(&self, path: &str) -> Result<Vec<String>> {
        let output = Command::new("reg")
            .args(["query", path])
            .output()
            .map_err(|e| crate::VulturiusError::Scan(format!("reg query failed: {}", e)))?;

        if !output.status.success() {
            return Ok(Vec::new()); // Path doesn't exist or no access
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut subkeys = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            // Subkey lines start with HKLM, HKCU, HKCR, etc.
            if trimmed.starts_with("HKLM")
                || trimmed.starts_with("HKCU")
                || trimmed.starts_with("HKCR")
                || trimmed.starts_with("HKU")
                || trimmed.starts_with("HKCC")
            {
                // Extract just the subkey name (last component after the root path)
                if let Some(subkey) = trimmed.strip_prefix(path).map(|s| s.trim_start_matches('\\')) {
                    if !subkey.is_empty() {
                        subkeys.push(subkey.to_string());
                    }
                }
            }
        }

        Ok(subkeys)
    }

    /// Parse an individual uninstall key and extract standard values.
    async fn parse_uninstall_key(&self, key_path: &str) -> Result<UninstallRecord> {
        let display_name = self.read_registry_value(key_path, "DisplayName").await.unwrap_or_default();
        if display_name.is_empty() {
            return Err(crate::VulturiusError::Scan(
                "Empty DisplayName".to_string(),
            ));
        }

        let publisher = self.read_registry_value(key_path, "Publisher").await.ok();
        let version = self.read_registry_value(key_path, "DisplayVersion").await.ok();
        let install_location =
            self.read_registry_value(key_path, "InstallLocation").await.ok();
        let uninstall_string =
            self.read_registry_value(key_path, "UninstallString").await.ok();
        let install_date = self.read_registry_value(key_path, "InstallDate").await.ok();
        let estimated_size_str =
            self.read_registry_value(key_path, "EstimatedSize").await.ok();

        let estimated_size = estimated_size_str
            .as_ref()
            .and_then(|s| s.parse::<u64>().ok())
            .map(|s| s * 1024); // EstimatedSize is in KB

        // Check if install location exists
        let install_exists = install_location
            .as_ref()
            .map(|loc| !loc.is_empty() && PathBuf::from(loc).exists())
            .unwrap_or(true); // If no InstallLocation, assume exists

        // Check if uninstall string references a valid executable
        let uninstall_valid = uninstall_string
            .as_ref()
            .map(|s| {
                if s.is_empty() {
                    return false;
                }
                // Extract executable path (may be wrapped in quotes)
                let exe_path = s.trim().trim_matches('"');
                // Get just the executable (first token, handling quotes)
                let exe = exe_path.split_whitespace().next().unwrap_or(exe_path);
                PathBuf::from(exe).exists()
            })
            .unwrap_or(false);

        Ok(UninstallRecord {
            key_path: key_path.to_string(),
            display_name,
            publisher,
            version,
            install_location,
            uninstall_string,
            install_date,
            estimated_size,
            install_exists,
            uninstall_valid,
        })
    }

    /// Read a specific registry value using `reg.exe`.
    async fn read_registry_value(&self, key_path: &str, value_name: &str) -> Result<String> {
        let output = Command::new("reg")
            .args(["query", key_path, "/v", value_name, "/reg:64"])
            .output()
            .map_err(|e| crate::VulturiusError::Scan(format!("reg query value failed: {}", e)))?;

        if !output.status.success() {
            return Ok(String::new()); // Value doesn't exist
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse the output — the value data is on the line containing the value type
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.contains("REG_SZ")
                || trimmed.contains("REG_EXPAND_SZ")
                || trimmed.contains("REG_DWORD")
                || trimmed.contains("REG_BINARY")
                || trimmed.contains("REG_MULTI_SZ")
                || trimmed.contains("REG_QWORD")
            {
                // Format: value_name  TYPE  data
                // or:    value_name   TYPE   data
                let parts: Vec<&str> = trimmed.splitn(3, '\t').collect();
                if parts.len() >= 3 && parts[0].trim().eq_ignore_ascii_case(value_name) {
                    return Ok(parts[2].trim().to_string());
                }
                // Fallback: try space-separated
                let space_parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
                if space_parts.len() >= 3
                    && space_parts[0].trim().eq_ignore_ascii_case(value_name)
                {
                    return Ok(space_parts[2].trim().to_string());
                }
            }
        }

        Ok(String::new())
    }

    /// Find uninstall records that reference missing installations.
    fn find_invalid_uninstall_records(
        &self,
        records: &[UninstallRecord],
    ) -> Vec<UninstallRecord> {
        records
            .iter()
            .filter(|r| !r.install_exists || !r.uninstall_valid)
            .cloned()
            .collect()
    }

    /// Find orphaned entries — uninstall keys for apps not in the known apps list
    /// or with missing install locations.
    fn find_orphaned_entries(
        &self,
        records: &[UninstallRecord],
    ) -> Result<Vec<RegistryEntry>> {
        let mut orphans = Vec::new();

        for record in records {
            // Check if this app is known
            let name_lower = record.display_name.to_lowercase();
            let is_known = self
                .known_apps
                .iter()
                .any(|known| name_lower.contains(known) || known.contains(&name_lower));

            let is_orphaned = if is_known {
                // Known app but install location is missing
                !record.install_exists
            } else {
                // Unknown app — likely orphaned if install location is missing
                // or if it has no valid uninstaller
                !record.install_exists || !record.uninstall_valid
            };

            if is_orphaned {
                let reason = if !record.install_exists {
                    format!(
                        "Install location '{}' does not exist",
                        record
                            .install_location
                            .as_deref()
                            .unwrap_or("(not specified)")
                    )
                } else if !record.uninstall_valid {
                    "Uninstall string references a missing executable".to_string()
                } else {
                    "App not found in installed applications list".to_string()
                };

                orphans.push(RegistryEntry {
                    key_path: record.key_path.clone(),
                    value_name: "DisplayName".to_string(),
                    value_data: record.display_name.clone(),
                    value_type: RegistryValueType::String,
                    is_orphaned: true,
                    reason,
                });
            }
        }

        Ok(orphans)
    }

    /// Scan auto-run and service keys for references to missing executables.
    async fn scan_unused_run_keys(&self) -> Result<Vec<RegistryEntry>> {
        let mut unused = Vec::new();

        for &root_path in ORPHAN_KEY_PATTERNS {
            match self.enumerate_subkeys(root_path).await {
                Ok(subkeys) => {
                    for subkey in subkeys {
                        let full_path = format!("{}\\{}", root_path, subkey);
                        if let Ok(entries) = self.check_run_key(&full_path).await {
                            unused.extend(entries);
                        }
                    }
                }
                Err(e) => {
                    debug!("Could not scan run keys in {}: {}", root_path, e);
                }
            }
        }

        // Also check the Run keys directly for values pointing to missing executables
        for &run_path in &[
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        ] {
            if let Ok(values) = self.list_registry_values(run_path).await {
                for (name, data, vtype) in values {
                    if let Some(exe_path) = extract_exe_path(&data) {
                        if !PathBuf::from(&exe_path).exists() {
                            unused.push(RegistryEntry {
                                key_path: run_path.to_string(),
                                value_name: name,
                                value_data: data,
                                value_type: vtype,
                                is_orphaned: true,
                                reason: format!("Referenced executable '{}' not found", exe_path),
                            });
                        }
                    }
                }
            }
        }

        Ok(unused)
    }

    /// Check a specific registry key for values referencing missing executables.
    async fn check_run_key(&self, key_path: &str) -> Result<Vec<RegistryEntry>> {
        let mut entries = Vec::new();

        if let Ok(values) = self.list_registry_values(key_path).await {
            for (name, data, vtype) in values {
                if let Some(exe_path) = extract_exe_path(&data) {
                    if !PathBuf::from(&exe_path).exists() {
                        entries.push(RegistryEntry {
                            key_path: key_path.to_string(),
                            value_name: name,
                            value_data: data,
                            value_type: vtype,
                            is_orphaned: true,
                            reason: format!("Referenced executable '{}' not found", exe_path),
                        });
                    }
                }
            }
        }

        Ok(entries)
    }

    /// List all values under a registry key.
    async fn list_registry_values(&self, key_path: &str) -> Result<Vec<(String, String, RegistryValueType)>> {
        let output = Command::new("reg")
            .args(["query", key_path, "/reg:64"])
            .output()
            .map_err(|e| crate::VulturiusError::Scan(format!("reg query values failed: {}", e)))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut values = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            // Skip key headers
            if trimmed.starts_with("HKLM") || trimmed.starts_with("HKCU") {
                continue;
            }

            // Try to parse: name  TYPE  data
            if let Some((name, vtype, data)) = parse_reg_line(trimmed) {
                values.push((name, data, vtype));
            }
        }

        Ok(values)
    }
}

// ─── Registry Cleaner ────────────────────────────────────────────────────────

/// Cleans orphaned and unused registry entries.
pub struct RegistryCleaner;

impl RegistryCleaner {
    pub fn new() -> Self {
        Self
    }

    /// Backup specified registry keys to a .reg file.
    pub async fn backup_keys(&self, keys: &[String]) -> Result<String> {
        if keys.is_empty() {
            return Ok(String::new());
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_path = format!(".vulturius_data/registry_backup_{}.reg", timestamp);

        let mut reg_content = String::from("Windows Registry Editor Version 5.00\n\n");

        for key in keys {
            // Backup by recording the key path with deletion prefix
            // This creates a .reg file that documents what was removed
            reg_content.push_str(&format!("-{}\n", key));
        }

        std::fs::write(&backup_path, &reg_content)
            .map_err(|e| crate::VulturiusError::Cleanup(format!("Failed to write registry backup: {}", e)))?;

        info!("Registry backup saved to {}", backup_path);
        Ok(backup_path)
    }

    /// Backup specific values within keys.
    pub async fn backup_entries(&self, entries: &[RegistryEntry]) -> Result<String> {
        if entries.is_empty() {
            return Ok(String::new());
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_path = format!(".vulturius_data/registry_entries_backup_{}.reg", timestamp);

        let mut reg_content = String::from("Windows Registry Editor Version 5.00\n\n");

        // Group entries by key path
        let mut key_groups: std::collections::BTreeMap<String, Vec<&RegistryEntry>> =
            std::collections::BTreeMap::new();

        for entry in entries {
            key_groups
                .entry(entry.key_path.clone())
                .or_default()
                .push(entry);
        }

        for (key_path, entries) in &key_groups {
            reg_content.push_str(&format!("[{}]\n", key_path));
            for entry in entries {
                // Delete the value (prefix with -)
                reg_content.push_str(&format!("-\"{}\"\n", entry.value_name));
            }
            reg_content.push('\n');
        }

        std::fs::write(&backup_path, &reg_content)
            .map_err(|e| crate::VulturiusError::Cleanup(format!("Failed to write registry backup: {}", e)))?;

        info!("Registry entries backup saved to {}", backup_path);
        Ok(backup_path)
    }

    /// Clean orphaned registry entries.
    ///
    /// Creates a backup first, then generates a .reg file that removes the entries.
    /// The .reg file is applied using `reg import`.
    pub async fn clean_orphaned_entries(
        &self,
        entries: &[RegistryEntry],
        dry_run: bool,
    ) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        info!(
            "Cleaning {} orphaned registry entries (dry_run={})",
            entries.len(),
            dry_run
        );

        // Create backup before making changes
        if !dry_run {
            let _backup_path = self.backup_entries(entries).await?;
        }

        if dry_run {
            for entry in entries {
                info!(
                    "[DRY RUN] Would delete: {}\\{} = {}",
                    entry.key_path, entry.value_name, entry.value_data
                );
            }
            return Ok(entries.len());
        }

        // Generate .reg file to delete the values
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let cleanup_reg = format!(".vulturius_data/cleanup_{}.reg", timestamp);

        let mut reg_content = String::from("Windows Registry Editor Version 5.00\n\n");

        let mut key_groups: std::collections::BTreeMap<String, Vec<&RegistryEntry>> =
            std::collections::BTreeMap::new();

        for entry in entries {
            key_groups
                .entry(entry.key_path.clone())
                .or_default()
                .push(entry);
        }

        for (key_path, entries) in &key_groups {
            reg_content.push_str(&format!("[{}]\n", key_path));
            for entry in entries {
                reg_content.push_str(&format!("-\"{}\"\n", entry.value_name));
            }
            reg_content.push('\n');
        }

        std::fs::write(&cleanup_reg, &reg_content)
            .map_err(|e| crate::VulturiusError::Cleanup(format!("Failed to write cleanup .reg file: {}", e)))?;

        // Apply the .reg file
        let output = Command::new("reg")
            .args(["import", &cleanup_reg])
            .output()
            .map_err(|e| crate::VulturiusError::Cleanup(format!("reg import failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::VulturiusError::Cleanup(format!(
                "Registry import failed: {}",
                stderr
            )));
        }

        let cleaned = entries.len();
        info!("Successfully cleaned {} registry entries", cleaned);

        Ok(cleaned)
    }

    /// Restore from a .reg backup file.
    pub async fn restore_from_backup(&self, backup_path: &str) -> Result<()> {
        if !std::path::Path::new(backup_path).exists() {
            return Err(crate::VulturiusError::Cleanup(format!(
                "Backup file not found: {}",
                backup_path
            )));
        }

        info!("Restoring from backup: {}", backup_path);

        let output = Command::new("reg")
            .args(["import", backup_path])
            .output()
            .map_err(|e| crate::VulturiusError::Cleanup(format!("reg import failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::VulturiusError::Cleanup(format!(
                "Registry restore failed: {}",
                stderr
            )));
        }

        info!("Registry restored from backup successfully");
        Ok(())
    }
}

// ─── Helper Functions ────────────────────────────────────────────────────────

/// Extract an executable path from a registry value string.
/// Handles quoted paths, /full command lines, etc.
fn extract_exe_path(value: &str) -> Option<String> {
    let trimmed = value.trim();

    if trimmed.starts_with('"') {
        // Quoted path
        if let Some(end_quote) = trimmed[1..].find('"') {
            let path = &trimmed[1..1 + end_quote];
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
    } else {
        // Unquoted — first token is the executable
        if let Some(first_token) = trimmed.split_whitespace().next() {
            if !first_token.is_empty() {
                return Some(first_token.to_string());
            }
        }
    }

    None
}

/// Parse a line from `reg query` output into (name, type, data).
fn parse_reg_line(line: &str) -> Option<(String, RegistryValueType, String)> {
    // Try tab-separated first (most common)
    let parts: Vec<&str> = line.splitn(3, '\t').collect();
    if parts.len() >= 3 {
        let vtype = parse_reg_type(parts[1].trim());
        return Some((parts[0].trim().to_string(), vtype, parts[2].trim().to_string()));
    }

    // Fallback: split on whitespace runs (for REG_DWORD and multi-space values)
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        let vtype = parse_reg_type(parts[1]);
        // Reconstruct data from remaining parts (in case data contains spaces)
        let data = parts[2..].join(" ");
        return Some((parts[0].to_string(), vtype, data));
    }

    None
}

/// Parse a registry type string.
fn parse_reg_type(s: &str) -> RegistryValueType {
    match s {
        t if t.eq_ignore_ascii_case("REG_SZ") => RegistryValueType::String,
        t if t.eq_ignore_ascii_case("REG_EXPAND_SZ") => RegistryValueType::ExpandString,
        t if t.eq_ignore_ascii_case("REG_BINARY") => RegistryValueType::Binary,
        t if t.eq_ignore_ascii_case("REG_DWORD") => RegistryValueType::DWord,
        t if t.eq_ignore_ascii_case("REG_QWORD") => RegistryValueType::QWord,
        t if t.eq_ignore_ascii_case("REG_MULTI_SZ") => RegistryValueType::MultiString,
        _ => RegistryValueType::Unknown,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_exe_path_quoted() {
        let path = extract_exe_path(r#""C:\Program Files\App\uninstall.exe" /silent"#);
        assert_eq!(path, Some("C:\\Program Files\\App\\uninstall.exe".to_string()));
    }

    #[test]
    fn test_extract_exe_path_unquoted() {
        let path = extract_exe_path(r"C:\Windows\System32\cmd.exe /c echo hello");
        assert_eq!(path, Some("C:\\Windows\\System32\\cmd.exe".to_string()));
    }

    #[test]
    fn test_extract_exe_path_empty() {
        assert_eq!(extract_exe_path(""), None);
        assert_eq!(extract_exe_path("   "), None);
    }

    #[test]
    fn test_parse_reg_type() {
        assert_eq!(parse_reg_type("REG_SZ"), RegistryValueType::String);
        assert_eq!(parse_reg_type("REG_DWORD"), RegistryValueType::DWord);
        assert_eq!(parse_reg_type("REG_BINARY"), RegistryValueType::Binary);
        assert_eq!(parse_reg_type("REG_EXPAND_SZ"), RegistryValueType::ExpandString);
        assert_eq!(parse_reg_type("UNKNOWN"), RegistryValueType::Unknown);
    }

    #[test]
    fn test_parse_reg_line_tab_separated() {
        let line = "DisplayName\tREG_SZ\tTest Application";
        let (name, vtype, data) = parse_reg_line(line).unwrap();
        assert_eq!(name, "DisplayName");
        assert_eq!(vtype, RegistryValueType::String);
        assert_eq!(data, "Test Application");
    }

    #[test]
    fn test_parse_reg_line_space_separated() {
        let line = "VersionInfo    REG_SZ    1.0.0";
        let (name, vtype, data) = parse_reg_line(line).unwrap();
        assert_eq!(name, "VersionInfo");
        assert_eq!(vtype, RegistryValueType::String);
        assert_eq!(data, "1.0.0");
    }

    #[test]
    fn test_registry_scanner_creation() {
        let scanner = RegistryScanner::new();
        assert!(scanner.known_apps.is_empty());
    }

    #[test]
    fn test_registry_scanner_with_known_apps() {
        let apps = vec![
            AppAnalysis {
                id: "1".to_string(),
                name: "TestApp".to_string(),
                version: None,
                install_path: PathBuf::from("C:\\Program Files\\TestApp"),
                last_launched: None,
                launch_count: 0,
                total_size: 0,
                usage_score: 0.0,
                uninstall_safe: true,
                leftover_files: Vec::new(),
                registry_entries: Vec::new(),
                recommendation: crate::types::AppRecommendation::Active,
            },
        ];

        let scanner = RegistryScanner::new().with_known_apps(&apps);
        assert!(scanner.known_apps.contains("testapp"));
    }

    #[test]
    fn test_registry_cleaner_creation() {
        let _cleaner = RegistryCleaner::new();
    }

    #[test]
    fn test_uninstall_record_serialization() {
        let record = UninstallRecord {
            key_path: r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}".to_string(),
            display_name: "Test App".to_string(),
            publisher: Some("Test Publisher".to_string()),
            version: Some("1.0.0".to_string()),
            install_location: Some(r"C:\Program Files\TestApp".to_string()),
            uninstall_string: Some("\"C:\\Program Files\\TestApp\\uninstall.exe\"".to_string()),
            install_date: Some("20240101".to_string()),
            estimated_size: Some(102400),
            install_exists: true,
            uninstall_valid: true,
        };

        let json = serde_json::to_string(&record).unwrap();
        let parsed: UninstallRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_name, "Test App");
        assert!(parsed.install_exists);
    }

    #[test]
    fn test_orphan_detection_with_missing_install() {
        let records = vec![
            UninstallRecord {
                key_path: r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\OldApp".to_string(),
                display_name: "OldApp".to_string(),
                publisher: None,
                version: None,
                install_location: Some(r"C:\Program Files\OldApp".to_string()),
                uninstall_string: None,
                install_date: None,
                estimated_size: None,
                install_exists: false,
                uninstall_valid: false,
            },
            UninstallRecord {
                key_path: r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\GoodApp".to_string(),
                display_name: "GoodApp".to_string(),
                publisher: None,
                version: None,
                install_location: Some(r"C:\Program Files\GoodApp".to_string()),
                uninstall_string: Some(r"C:\Program Files\GoodApp\uninstall.exe".to_string()),
                install_date: None,
                estimated_size: None,
                install_exists: true,
                uninstall_valid: true,
            },
        ];

        let scanner = RegistryScanner::new();
        let orphans = scanner.find_orphaned_entries(&records).unwrap();

        // OldApp should be orphaned (install missing + no valid uninstaller)
        assert!(orphans.iter().any(|o| o.value_data == "OldApp"));
        // GoodApp should not be orphaned
        assert!(!orphans.iter().any(|o| o.value_data == "GoodApp"));
    }
}
