use crate::config::AppConfig;
use crate::platform::create_platform_ops;
use crate::types::*;
use crate::VulturiusError;

use std::time::SystemTime;

pub mod app_detector;
pub use app_detector::AppDetector;

/// Analyzes scan results and generates cleanup recommendations.
pub struct Analyzer {
    config: AppConfig,
    platform: Box<dyn crate::platform::PlatformOps>,
}

impl Analyzer {
    pub fn new(config: AppConfig) -> Self {
        Self {
            platform: create_platform_ops(),
            config,
        }
    }

    /// Analyze a scan result and produce file-level recommendations.
    pub fn analyze(&self, scan_result: &ScanResult) -> Vec<FileAnalysis> {
        let now = SystemTime::now();
        let risk_tolerance = &self.config.analysis.risk_tolerance;
        let min_score = risk_tolerance.min_score();

        scan_result
            .entries
            .iter()
            .filter(|entry| entry.file_type == FileType::RegularFile)
            .map(|entry| self.analyze_file(entry, now, min_score))
            .collect()
    }

    /// Analyze a single file entry.
    fn analyze_file(
        &self,
        entry: &FileEntry,
        now: SystemTime,
        min_score: f64,
    ) -> FileAnalysis {
        let breakdown = self.compute_score_breakdown(entry, now);
        let confidence = self.weighted_score(&breakdown);

        let recommendation = if confidence >= min_score {
            if confidence >= 0.90 {
                Recommendation::SafeToDelete
            } else {
                Recommendation::Caution
            }
        } else if confidence >= min_score * 0.5 {
            Recommendation::ReviewManually
        } else {
            Recommendation::DoNotDelete
        };

        let reason = self.generate_reason(entry, &breakdown, confidence);

        FileAnalysis {
            file_id: entry.id.clone(),
            path: entry.path.clone(),
            confidence_score: confidence,
            recommendation,
            reason,
            score_breakdown: breakdown,
            llm_reviewed: false,
        }
    }

    /// Compute individual score factors.
    fn compute_score_breakdown(&self, entry: &FileEntry, now: SystemTime) -> ScoreBreakdown {
        // Age factor: older = higher score
        let age_days = entry.age_days(now).unwrap_or(0);
        let age_factor = (age_days as f64 / 365.0).min(1.0);

        // Location factor: temp/cache dirs = higher score
        let path_str = entry.path.to_string_lossy().to_lowercase();
        let location_factor = if path_str.contains("temp") || path_str.contains("cache") || path_str.starts_with("/tmp/") {
            0.95
        } else if path_str.contains("log") {
            0.80
        } else {
            0.10
        };

        // Type factor: based on file category
        let type_factor = entry.category.default_safety_score();

        // Dependency factor: placeholder (would check if anything references this file)
        let dependency_factor = 0.50;

        // User pattern factor: placeholder (would check user's historical behavior)
        let user_pattern_factor = 0.50;

        ScoreBreakdown {
            age_factor,
            location_factor,
            type_factor,
            dependency_factor,
            user_pattern_factor,
            llm_bonus: 0.0,
        }
    }

    /// Compute weighted confidence score from breakdown.
    fn weighted_score(&self, breakdown: &ScoreBreakdown) -> f64 {
        let age_weight = 0.30;
        let location_weight = 0.25;
        let type_weight = 0.20;
        let dependency_weight = 0.15;
        let user_pattern_weight = 0.10;

        let score = breakdown.age_factor * age_weight
            + breakdown.location_factor * location_weight
            + breakdown.type_factor * type_weight
            + breakdown.dependency_factor * dependency_weight
            + breakdown.user_pattern_factor * user_pattern_weight;

        // Clamp to [0.0, 1.0]
        score.clamp(0.0, 1.0)
    }

    /// Generate a human-readable reason for the recommendation.
    fn generate_reason(
        &self,
        entry: &FileEntry,
        _breakdown: &ScoreBreakdown,
        confidence: f64,
    ) -> String {
        let age_days = entry
            .age_days(SystemTime::now())
            .map(|d| d.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        format!(
            "Category: {}, Age: {} days, Confidence: {:.0}%",
            entry.category,
            age_days,
            confidence * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use std::time::{Duration, SystemTime};

    fn test_scan_result() -> ScanResult {
        ScanResult {
            scan_id: "test".to_string(),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            entries: vec![
                FileEntry {
                    id: "1".to_string(),
                    path: std::path::PathBuf::from("/tmp/old_file.tmp"),
                    size: 1024,
                    file_type: FileType::RegularFile,
                    category: FileCategory::Temp,
                    last_accessed: SystemTime::now() - Duration::from_secs(86400 * 100),
                    last_modified: SystemTime::now() - Duration::from_secs(86400 * 100),
                    created: SystemTime::now() - Duration::from_secs(86400 * 200),
                    permissions: 0o644,
                    is_system: false,
                    is_hidden: false,
                    mime_type: None,
                    hash: None,
                },
                FileEntry {
                    id: "2".to_string(),
                    path: std::path::PathBuf::from("/home/user/documents/report.pdf"),
                    size: 2048,
                    file_type: FileType::RegularFile,
                    category: FileCategory::Document,
                    last_accessed: SystemTime::now() - Duration::from_secs(86400),
                    last_modified: SystemTime::now() - Duration::from_secs(86400),
                    created: SystemTime::now() - Duration::from_secs(86400 * 30),
                    permissions: 0o644,
                    is_system: false,
                    is_hidden: false,
                    mime_type: Some("application/pdf".to_string()),
                    hash: None,
                },
            ],
            total_size: 3072,
            total_files: 2,
            total_dirs: 0,
            errors: vec![],
            scan_type: ScanType::Quick,
        }
    }

    #[test]
    fn test_analyzer_temp_file_high_confidence() {
        let config = AppConfig::default();
        let analyzer = Analyzer::new(config);
        let scan = test_scan_result();
        let results = analyzer.analyze(&scan);

        let temp_analysis = results.iter().find(|r| r.file_id == "1").unwrap();
        assert!(temp_analysis.confidence_score > 0.5);
        assert!(matches!(temp_analysis.recommendation, Recommendation::SafeToDelete | Recommendation::Caution));
    }

    #[test]
    fn test_analyzer_document_low_confidence() {
        let config = AppConfig::default();
        let analyzer = Analyzer::new(config);
        let scan = test_scan_result();
        let results = analyzer.analyze(&scan);

        let doc_analysis = results.iter().find(|r| r.file_id == "2").unwrap();
        assert!(doc_analysis.confidence_score < 0.5);
        assert_ne!(doc_analysis.recommendation, Recommendation::SafeToDelete);
    }
}
