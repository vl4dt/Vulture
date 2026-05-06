//! Benchmark helper data for Vulturius performance testing.

use std::path::PathBuf;
use std::time::SystemTime;

use crate::types::{FileCategory, FileEntry, FileType};

/// Generate a synthetic FileEntry for benchmarking.
pub fn make_file_entry(id: usize, category: FileCategory, size: u64) -> FileEntry {
    FileEntry {
        id: format!("bench-{id:06}"),
        path: PathBuf::from(format!("/bench/data/file_{id:06}.dat")),
        size,
        file_type: FileType::RegularFile,
        category,
        last_accessed: SystemTime::now(),
        last_modified: SystemTime::now(),
        created: SystemTime::now(),
        permissions: 0o644,
        is_system: false,
        is_hidden: false,
        mime_type: Some("application/octet-stream".to_string()),
        hash: None,
    }
}

/// Generate a batch of synthetic FileEntries.
pub fn make_file_entries(count: usize) -> Vec<FileEntry> {
    let categories = [
        FileCategory::Temp,
        FileCategory::Cache,
        FileCategory::Log,
        FileCategory::Document,
        FileCategory::Image,
        FileCategory::Unknown,
    ];

    (0..count)
        .map(|i| {
            let cat = categories[i % categories.len()].clone();
            let size = 1024 * (1 + (i % 1024));
            make_file_entry(i, cat, size as u64)
        })
        .collect()
}

/// Test paths for classifier benchmarks.
pub fn classifier_test_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/tmp/file.tmp"),
        PathBuf::from("/var/log/syslog.log"),
        PathBuf::from(".cache/data.cache"),
        PathBuf::from("photos/vacation.jpg"),
        PathBuf::from("docs/report.pdf"),
        PathBuf::from("videos/movie.mp4"),
        PathBuf::from("music/track.mp3"),
        PathBuf::from("config.bak"),
        PathBuf::from("data.txt"),
        PathBuf::from("app.exe"),
        PathBuf::from("lib/library.dll"),
        PathBuf::from("settings.json"),
        PathBuf::from("crash.dmp"),
        PathBuf::from("thumbs.db"),
        PathBuf::from("download.part"),
        PathBuf::from("/tmp/session_12345"),
        PathBuf::from("C:\\Users\\test\\AppData\\Local\\Temp\\data"),
        PathBuf::from("C:\\Users\\test\\AppData\\Local\\Google\\Chrome\\cache"),
        PathBuf::from("/var/cache/apt/packages"),
        PathBuf::from("some/random/unknown/file"),
    ]
}
