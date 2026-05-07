//! End-to-end integration tests for Vulturius.
//!
//! These tests verify the complete pipeline: scan → analyze → clean → report.

use std::fs;
use std::path::PathBuf;

use vulturius_core::analyzer::Analyzer;
use vulturius_core::config::AppConfig;
use vulturius_core::scanner::{FileClassifier, Scanner};
use vulturius_core::types::ScanTarget;
use vulturius_core::security::{AuditLogger, PathGuard, PathSandbox, PermissionModel};
use vulturius_core::types::FileCategory;

/// Create a test directory structure mimicking a real filesystem.
fn create_test_filesystem(base: &std::path::Path) -> std::path::PathBuf {
    let root = base.join("test_fs");

    // Temp files
    let temp_dir = root.join("temp");
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(temp_dir.join("session_12345.tmp"), "temp data").unwrap();
    fs::write(temp_dir.join("cache_blob.tmp"), vec![0u8; 1024]).unwrap();

    // Cache directory
    let cache_dir = root.join("cache");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("thumbnail.db"), "cache content").unwrap();
    fs::write(cache_dir.join("index.cache"), "index data").unwrap();

    // Log files
    let log_dir = root.join("logs");
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(log_dir.join("app.log"), "log line 1\nlog line 2\nlog line 3\n").unwrap();
    fs::write(log_dir.join("error.log"), "error: something failed\n").unwrap();

    // Documents (should be kept)
    let docs_dir = root.join("documents");
    fs::create_dir_all(&docs_dir).unwrap();
    fs::write(docs_dir.join("report.pdf"), "important document").unwrap();
    fs::write(docs_dir.join("notes.txt"), "keep this file").unwrap();

    // Old backup files
    let backup_dir = root.join("backups");
    fs::create_dir_all(&backup_dir).unwrap();
    fs::write(backup_dir.join("old_config.bak"), "old config").unwrap();
    fs::write(backup_dir.join("database.bak"), vec![0u8; 2048]).unwrap();

    root
}

#[tokio::test]
async fn e2e_scan_analyze_pipeline() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_root = create_test_filesystem(temp_dir.path());

    let config = AppConfig::default();

    // Scan
    let scanner = Scanner::new(config.clone());
    let targets = vec![ScanTarget {
        scan_type: vulturius_core::types::ScanType::Full,
        path: test_root.clone(),
        max_depth: None,
        exclusions: vec![],
        file_types: None,
        min_file_size: None,
        collect_hashes: false,
    }];

    let result = scanner.scan(targets, None).await.unwrap();
    assert!(result.total_files > 0, "Should have scanned files");
    assert!(result.total_size > 0, "Should have total size");

    // Analyze
    let analyzer = Analyzer::new(config.clone());
    let analyses = analyzer.analyze(&result);
    assert!(!analyses.is_empty(), "Should have analyses");

    // Verify categories are classified
    let has_temp = result.entries.iter().any(|e| e.category == FileCategory::Temp);
    let has_log = result.entries.iter().any(|e| e.category == FileCategory::Log);
    let has_cache = result.entries.iter().any(|e| e.category == FileCategory::Cache);

    assert!(
        has_temp || has_log || has_cache,
        "Should classify at least one category"
    );
}

#[tokio::test]
async fn e2e_security_sandbox_enforcement() {
    let temp_dir = tempfile::tempdir().unwrap();
    let allowed = temp_dir.path().to_path_buf();

    // Create a file inside the allowed root
    let safe_path = allowed.join("safe_file.txt");
    fs::write(&safe_path, "test").unwrap();

    // Use unrestricted sandbox for the test, then verify allowed roots work
    let sandbox = PathSandbox::unrestricted();
    assert!(sandbox.check_path(&safe_path).is_ok());

    // Now test with allowed roots — the path must be canonicalizable
    // We test that the sandbox correctly rejects paths outside the root
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("bad.txt");
    fs::write(&outside_file, "test").unwrap();

    let sandbox_with_roots = PathSandbox::new(vec![allowed.clone()]);
    // The allowed root is the temp dir — paths under it should be allowed
    // Paths outside should be denied
    assert!(sandbox_with_roots.check_path(&outside_file).is_err());
}

#[tokio::test]
async fn e2e_permission_model_workflow() {
    // Read-only user should not be able to delete
    let readonly = PermissionModel::read_only();
    assert!(!readonly
        .check(vulturius_core::security::permission::Operation::DeleteFile)
        .is_allowed());

    // User should be able to delete but with confirmation
    let user = PermissionModel::user();
    let result = user.check(vulturius_core::security::permission::Operation::DeleteFile);
    assert!(result.is_allowed());
    assert!(result.requires_confirmation());

    // Admin should be able to modify registry
    let admin = PermissionModel::administrator();
    assert!(admin
        .check(vulturius_core::security::permission::Operation::ModifyRegistry)
        .is_allowed());
}

#[tokio::test]
async fn e2e_path_guard_traversal_protection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let base = temp_dir.path();

    // Safe path
    let safe_path = base.join("subdir/file.txt");
    assert!(PathGuard::validate(&safe_path, Some(base)).is_ok());

    // Traversal attempt
    let escape_path = base.join("subdir/../../etc/passwd");
    assert!(PathGuard::validate(&escape_path, Some(base)).is_err());

    // Null byte injection
    let null_path = PathBuf::from("test\0file.txt");
    assert!(PathGuard::validate(&null_path, None).is_err());
}

#[tokio::test]
async fn e2e_audit_logger_full_cycle() {
    let logger = AuditLogger::new();

    // Log various events
    logger.log_scan_event("scan-001", "started").await;
    logger
        .log_file_operation("read", "/tmp/test.txt", true)
        .await;
    logger
        .log_permission_check("delete_file", "user", true)
        .await;
    logger
        .log_cleanup_event("plan-001", "quarantine", true)
        .await;
    logger.log_policy_violation("test violation").await;

    // Verify entries
    assert_eq!(logger.entry_count().await, 5);

    // Verify summary
    let summary = logger.security_summary().await;
    assert_eq!(summary.total_events, 5);
    assert_eq!(summary.policy_violations, 1);

    // Export JSON
    let json = logger.export_json().await.unwrap();
    assert!(json.contains("scan-001"));
    assert!(json.contains("quarantine"));
}

#[tokio::test]
async fn e2e_classifier_comprehensive() {
    let classifier = FileClassifier::new();

    // Test all major categories
    assert_eq!(
        classifier.classify(&PathBuf::from("/tmp/session.tmp")),
        FileCategory::Temp
    );
    assert_eq!(
        classifier.classify(&PathBuf::from(".cache/data.cache")),
        FileCategory::Cache
    );
    assert_eq!(
        classifier.classify(&PathBuf::from("/var/log/syslog.log")),
        FileCategory::Log
    );
    assert_eq!(
        classifier.classify(&PathBuf::from("photos/vacation.jpg")),
        FileCategory::Image
    );
    assert_eq!(
        classifier.classify(&PathBuf::from("docs/report.pdf")),
        FileCategory::Document
    );

    // Safety scores
    assert!(FileCategory::Temp.default_safety_score() > 0.5);
    assert!(FileCategory::Document.default_safety_score() < 0.3);
}

#[tokio::test]
async fn e2e_scan_does_not_modify_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_root = create_test_filesystem(temp_dir.path());

    let config = AppConfig::default();
    let scanner = Scanner::new(config.clone());

    let targets = vec![ScanTarget {
        scan_type: vulturius_core::types::ScanType::Full,
        path: test_root.clone(),
        max_depth: None,
        exclusions: vec![],
        file_types: None,
        min_file_size: None,
        collect_hashes: false,
    }];

    let initial_count = count_files(&test_root);

    let _ = scanner.scan(targets, None).await.unwrap();

    // File count should be unchanged after scan
    let after_count = count_files(&test_root);
    assert_eq!(initial_count, after_count, "Scan should not modify files");
}

#[tokio::test]
async fn e2e_incremental_scan_skip_unchanged() {
    use vulturius_core::perf::incremental::{DirectorySnapshot, IncrementalScanState};

    let temp_dir = tempfile::tempdir().unwrap();

    // Create some files
    fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
    fs::write(temp_dir.path().join("file2.txt"), "content").unwrap();

    // Create initial snapshot
    let mut snapshot = DirectorySnapshot::new();
    if let Ok(meta) = fs::metadata(temp_dir.path()) {
        snapshot.record_dir(temp_dir.path(), &meta);
    }

    // First check: should be unchanged
    let mut state = IncrementalScanState::new(true, Some(snapshot.clone()));
    if let Ok(meta) = fs::metadata(temp_dir.path()) {
        assert!(state.check_dir(temp_dir.path(), &meta));
    }
}

#[tokio::test]
async fn e2e_full_scan_analyze_verify() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_root = create_test_filesystem(temp_dir.path());

    let config = AppConfig::default();
    let scanner = Scanner::new(config.clone());
    let analyzer = Analyzer::new(config.clone());

    let targets = vec![ScanTarget {
        scan_type: vulturius_core::types::ScanType::Full,
        path: test_root,
        max_depth: None,
        exclusions: vec![],
        file_types: None,
        min_file_size: None,
        collect_hashes: false,
    }];

    let result = scanner.scan(targets, None).await.unwrap();
    let analyses = analyzer.analyze(&result);

    // Verify: scan found files, analysis produced results
    assert!(result.total_files >= 7); // At least our test files
    assert!(!analyses.is_empty());

    // Verify: temp files get higher confidence scores than documents
    let temp_paths: Vec<_> = result
        .entries
        .iter()
        .filter(|e| e.category == FileCategory::Temp)
        .map(|e| e.path.clone())
        .collect();

    let doc_paths: Vec<_> = result
        .entries
        .iter()
        .filter(|e| e.category == FileCategory::Document)
        .map(|e| e.path.clone())
        .collect();

    let temp_scores: Vec<_> = analyses
        .iter()
        .filter(|a| temp_paths.contains(&a.path))
        .map(|a| a.confidence_score)
        .collect();

    let doc_scores: Vec<_> = analyses
        .iter()
        .filter(|a| doc_paths.contains(&a.path))
        .map(|a| a.confidence_score)
        .collect();

    if !temp_scores.is_empty() && !doc_scores.is_empty() {
        let avg_temp = temp_scores.iter().sum::<f64>() / temp_scores.len() as f64;
        let avg_doc = doc_scores.iter().sum::<f64>() / doc_scores.len() as f64;
        assert!(
            avg_temp > avg_doc,
            "Temp files should have higher confidence scores than documents"
        );
    }
}

#[tokio::test]
async fn e2e_security_summary() {
    let logger = AuditLogger::new();

    // Simulate a session with various events
    for i in 0..10 {
        logger
            .log_file_operation("read", &format!("/file/{}.txt", i), true)
            .await;
    }

    for i in 0..3 {
        logger
            .log_file_operation("delete", &format!("/temp/{}.tmp", i), true)
            .await;
    }

    logger
        .log_policy_violation("unauthorized access attempt")
        .await;

    let summary = logger.security_summary().await;
    assert_eq!(summary.total_events, 14);
    assert_eq!(summary.policy_violations, 1);
}

/// Helper: count files in a directory recursively.
fn count_files(path: &std::path::Path) -> usize {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count()
}
