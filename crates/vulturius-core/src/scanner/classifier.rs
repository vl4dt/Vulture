use std::path::PathBuf;

use crate::types::FileCategory;

/// Classifies files into categories based on path, extension, and directory context.
pub struct FileClassifier;

impl FileClassifier {
    pub fn new() -> Self {
        Self
    }

    /// Classify a file based on its path.
    pub fn classify(&self, path: &PathBuf) -> FileCategory {
        let path_str = path.to_string_lossy().to_lowercase();
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase());

        // Check extension first (most reliable signal)
        if let Some(ext) = &extension {
            // Temp files
            if ["tmp", "temp", "tmpx"].contains(&ext.as_str()) {
                return FileCategory::Temp;
            }
            // Log files
            if ext == "log" || ext == "trace" {
                return FileCategory::Log;
            }
            // Cache files
            if ext == "cache" || ext == "cachefile" {
                return FileCategory::Cache;
            }
            // Crash dumps
            if ["dmp", "crash", "mdmp"].contains(&ext.as_str()) {
                return FileCategory::CrashDump;
            }
            // Thumbnails
            if ext == "thumb" || ext == "thumbnail" || path_str.contains("thumbs.db") {
                return FileCategory::Thumbnail;
            }
            // Download helpers
            if ["part", "crdownload", "incomplete", "download", "torrent"]
                .contains(&ext.as_str())
            {
                return FileCategory::DownloadHelper;
            }
            // Old backups
            if ["bak", "old", "backup", "orig", "rejected"].contains(&ext.as_str()) {
                return FileCategory::OldBackup;
            }
            // Documents
            if ["doc", "docx", "pdf", "txt", "rtf", "odt", "xls", "xlsx", "ppt", "pptx"]
                .contains(&ext.as_str())
            {
                return FileCategory::Document;
            }
            // Images
            if ["jpg", "jpeg", "png", "gif", "bmp", "svg", "webp", "tiff", "ico"].contains(&ext.as_str()) {
                return FileCategory::Image;
            }
            // Videos
            if ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"].contains(&ext.as_str()) {
                return FileCategory::Video;
            }
            // Audio
            if ["mp3", "wav", "flac", "aac", "ogg", "wma", "m4a"].contains(&ext.as_str()) {
                return FileCategory::Audio;
            }
            // Executables
            if ["exe", "msi", "bat", "cmd", "sh", "ps1", "appimage"].contains(&ext.as_str()) {
                return FileCategory::Executable;
            }
            // Libraries
            if ["dll", "so", "dylib", "a", "lib"].contains(&ext.as_str()) {
                return FileCategory::Library;
            }
            // Config
            if ["ini", "cfg", "conf", "config", "toml", "yaml", "yml", "json", "xml"]
                .contains(&ext.as_str())
            {
                return FileCategory::Config;
            }
        }

        // Check directory context
        self.classify_by_path(&path_str)
    }

    /// Classify based on directory path patterns.
    fn classify_by_path(&self, path_str: &str) -> FileCategory {
        // Temp directories
        if path_str.contains("\\temp\\")
            || path_str.contains("/temp/")
            || path_str.starts_with("/tmp/")
            || path_str.starts_with("/var/tmp/")
        {
            return FileCategory::Temp;
        }

        // Cache directories
        if path_str.contains("\\cache\\")
            || path_str.contains("/cache/")
            || path_str.contains(".cache/")
            || path_str.contains("\\caché\\")
        {
            return FileCategory::Cache;
        }

        // Log directories
        if path_str.contains("\\logs\\")
            || path_str.contains("/logs/")
            || path_str.starts_with("/var/log/")
        {
            return FileCategory::Log;
        }

        // Browser cache directories
        if path_str.contains("\\chrome\\")
            || path_str.contains("\\firefox\\")
            || path_str.contains("\\edge\\")
            || path_str.contains("\\safari\\")
        {
            return FileCategory::BrowserData;
        }

        // Package manager caches
        if path_str.contains("\\pip\\")
            || path_str.contains("\\npm\\")
            || path_str.contains("\\cargo\\")
            || path_str.contains("/var/cache/apt/")
            || path_str.contains("/var/cache/pacman/")
        {
            return FileCategory::PackageCache;
        }

        FileCategory::Unknown
    }
}

impl Default for FileClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_temp_files() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from("/tmp/file.tmp")),
            FileCategory::Temp
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("C:\\Users\\test\\AppData\\Local\\Temp\\data")),
            FileCategory::Temp
        );
    }

    #[test]
    fn test_classify_log_files() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from("/var/log/syslog.log")),
            FileCategory::Log
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("app/logs/error.log")),
            FileCategory::Log
        );
    }

    #[test]
    fn test_classify_cache_files() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from(".cache/thumbnails/cache")),
            FileCategory::Cache
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("data.cache")),
            FileCategory::Cache
        );
    }

    #[test]
    fn test_classify_media_files() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from("photo.jpg")),
            FileCategory::Image
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("video.mp4")),
            FileCategory::Video
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("song.mp3")),
            FileCategory::Audio
        );
    }

    #[test]
    fn test_classify_document_files() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from("report.pdf")),
            FileCategory::Document
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("spreadsheet.xlsx")),
            FileCategory::Document
        );
    }

    #[test]
    fn test_classify_backup_files() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from("config.bak")),
            FileCategory::OldBackup
        );
        assert_eq!(
            classifier.classify(&PathBuf::from("data.old")),
            FileCategory::OldBackup
        );
    }

    #[test]
    fn test_classify_unknown() {
        let classifier = FileClassifier::new();
        assert_eq!(
            classifier.classify(&PathBuf::from("/some/random/path/data")),
            FileCategory::Unknown
        );
    }
}
