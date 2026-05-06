//! Desktop notification service.
//!
//! Uses Tauri notification plugin when available, falls back to
//! platform-native notification methods.

use tracing::{info, warn};

/// Notification type for styling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationType {
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationType {
    /// Get the icon name for this notification type.
    pub fn icon(&self) -> &str {
        match self {
            NotificationType::Info => "info",
            NotificationType::Success => "success",
            NotificationType::Warning => "warning",
            NotificationType::Error => "error",
        }
    }

    /// Get the urgency level.
    pub fn urgency(&self) -> &str {
        match self {
            NotificationType::Info => "normal",
            NotificationType::Success => "normal",
            NotificationType::Warning => "normal",
            NotificationType::Error => "critical",
        }
    }
}

/// Desktop notification service.
pub struct NotificationService;

impl NotificationService {
    /// Send a desktop notification.
    ///
    /// In a Tauri context this uses the notification plugin.
    /// In CLI/core context this logs the notification.
    pub async fn notify(
        _kind: NotificationType,
        _title: &str,
        _body: &str,
    ) {
        // In the core library we log notifications.
        // The Tauri GUI layer wraps this to use the notification plugin.
        info!("[NOTIFY] {}: {}", _title, _body);

        // Try to send via platform-native methods
        #[cfg(target_os = "windows")]
        Self::send_windows_notification(_title, _body);

        #[cfg(target_os = "macos")]
        Self::send_macos_notification(_title, _body);

        #[cfg(target_os = "linux")]
        Self::send_linux_notification(_title, _body);
    }

    /// Send a scan complete notification.
    pub async fn scan_complete(total_files: usize, total_size: u64) {
        Self::notify(
            NotificationType::Info,
            "Scan Complete",
            &format!("Scanned {} files ({})", total_files, format_size(total_size)),
        )
        .await;
    }

    /// Send a cleanup complete notification.
    pub async fn cleanup_complete(items: usize, freed: u64) {
        Self::notify(
            NotificationType::Success,
            "Cleanup Complete",
            &format!("Processed {} items, freed {}", items, format_size(freed)),
        )
        .await;
    }

    /// Send an error notification.
    pub async fn error(title: &str, message: &str) {
        Self::notify(NotificationType::Error, title, message).await;
    }

    /// Send a warning notification.
    pub async fn warning(title: &str, message: &str) {
        Self::notify(NotificationType::Warning, title, message).await;
    }

    // ── Platform-specific implementations ──

    #[cfg(target_os = "windows")]
    fn send_windows_notification(_title: &str, _body: &str) {
        // Windows uses Toast notifications via UWP
        // In practice, Tauri's notification plugin handles this
        // This is a fallback for non-Tauri contexts
        warn!("Windows toast notification: {} — {}", _title, _body);
    }

    #[cfg(target_os = "macos")]
    fn send_macos_notification(_title: &str, _body: &str) {
        // macOS uses `osascript` for notifications
        use std::process::Command;
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escape_applescript(_body),
            escape_applescript(_title),
        );
        let _ = Command::new("osascript")
            .args(["-e", &script])
            .status();
    }

    #[cfg(target_os = "linux")]
    fn send_linux_notification(_title: &str, _body: &str) {
        // Linux uses `notify-send`
        use std::process::Command;
        let _ = Command::new("notify-send")
            .args([_title, _body])
            .status();
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
impl NotificationService {
    fn send_windows_notification(_title: &str, _body: &str) {}
    fn send_macos_notification(_title: &str, _body: &str) {}
    fn send_linux_notification(_title: &str, _body: &str) {}
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000_000 {
        format!("{:.2} TB", bytes as f64 / 1_000_000_000_000.0)
    } else if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.2} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_type_icon() {
        assert_eq!(NotificationType::Info.icon(), "info");
        assert_eq!(NotificationType::Success.icon(), "success");
        assert_eq!(NotificationType::Warning.icon(), "warning");
        assert_eq!(NotificationType::Error.icon(), "error");
    }

    #[test]
    fn test_notification_type_urgency() {
        assert_eq!(NotificationType::Info.urgency(), "normal");
        assert_eq!(NotificationType::Success.urgency(), "normal");
        assert_eq!(NotificationType::Warning.urgency(), "normal");
        assert_eq!(NotificationType::Error.urgency(), "critical");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1500), "1.50 KB");
        assert_eq!(format_size(1_500_000), "1.50 MB");
        assert_eq!(format_size(1_500_000_000), "1.50 GB");
        assert_eq!(format_size(1_500_000_000_000), "1.50 TB");
    }

    #[test]
    fn test_escape_applescript() {
        assert_eq!(escape_applescript("hello \"world\""), "hello \\\"world\\\"");
        assert_eq!(escape_applescript("path\\to\\file"), "path\\\\to\\\\file");
    }
}
