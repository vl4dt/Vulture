//! LLM integration for augmented file analysis.
//!
//! Full implementation in Phase 5. This module provides the interface and stubs.

use crate::config::LlmConfig;
use crate::types::FileEntry;
use crate::Result;

use serde::{Deserialize, Serialize};

/// LLM analysis result for a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAnalysisResult {
    pub safe_to_delete: bool,
    pub confidence: f64,
    pub reason: String,
    pub category: String,
}

/// LLM engine for augmented analysis.
pub struct LlmEngine {
    config: LlmConfig,
    calls_made: usize,
}

impl LlmEngine {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            calls_made: 0,
        }
    }

    /// Analyze a file using the LLM.
    pub async fn analyze_file(&mut self, _entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        // TODO: Full implementation in Phase 5
        // - Check if LLM is enabled
        // - Check call budget
        // - Check cache
        // - Build prompt
        // - Call provider
        // - Parse response
        // - Cache result
        Ok(None)
    }

    /// Analyze a batch of files.
    pub async fn analyze_batch(
        &mut self,
        _entries: &[FileEntry],
    ) -> Result<Vec<LlmAnalysisResult>> {
        // TODO: Full implementation in Phase 5
        Ok(Vec::new())
    }

    /// Convert natural language query to scan configuration.
    pub async fn query_to_scan_config(&mut self, _query: &str) -> Result<String> {
        // TODO: Full implementation in Phase 5
        Ok(String::new())
    }

    /// Get the number of LLM calls made in current session.
    pub fn calls_made(&self) -> usize {
        self.calls_made
    }

    /// Check if we've exceeded the call budget.
    pub fn has_budget(&self) -> bool {
        self.config.enabled && self.calls_made < self.config.max_calls_per_scan
    }
}
